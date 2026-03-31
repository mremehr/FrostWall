use super::*;

fn history(max: usize) -> PairingHistory {
    PairingHistory::new(max)
}

fn paths(a: &str, b: &str) -> (PathBuf, PathBuf) {
    (PathBuf::from(a), PathBuf::from(b))
}

fn pairing(screens: &[(&str, &str)]) -> HashMap<String, PathBuf> {
    screens
        .iter()
        .map(|(screen, path)| (screen.to_string(), PathBuf::from(path)))
        .collect()
}

#[test]
fn base_score_zero_pairings_is_zero() {
    assert_eq!(PairingHistory::calculate_base_score(0, 0.0), 0.0);
}

#[test]
fn base_score_increases_with_pairings() {
    let s1 = PairingHistory::calculate_base_score(1, 0.0);
    let s5 = PairingHistory::calculate_base_score(5, 0.0);
    let s10 = PairingHistory::calculate_base_score(10, 0.0);
    assert!(s1 < s5 && s5 < s10, "score should grow with pair_count");
}

#[test]
fn base_score_capped_at_one() {
    let score = PairingHistory::calculate_base_score(1000, 9999.0);
    assert!(score <= 1.0, "score must not exceed 1.0, got {score}");
}

#[test]
fn base_score_duration_adds_bonus() {
    let no_duration = PairingHistory::calculate_base_score(3, 0.0);
    let long_duration = PairingHistory::calculate_base_score(3, 1800.0);
    assert!(
        long_duration > no_duration,
        "longer duration should boost score"
    );
}

#[test]
fn ordered_pair_is_stable_and_commutative() {
    let (a, b) = paths("/a/foo.jpg", "/b/bar.jpg");
    let (left, right) = PairingHistory::ordered_pair(&a, &b);
    let (left_again, right_again) = PairingHistory::ordered_pair(&b, &a);
    assert_eq!(
        (left, right),
        (left_again, right_again),
        "ordered_pair must be commutative"
    );
}

#[test]
fn ordered_pair_puts_lesser_path_first() {
    let (a, b) = paths("/alpha.jpg", "/zeta.jpg");
    let (first, _) = PairingHistory::ordered_pair(&a, &b);
    assert_eq!(first, Path::new("/alpha.jpg"));
}

#[test]
fn affinity_starts_at_zero() {
    let history = history(100);
    let (a, b) = paths("/a.jpg", "/b.jpg");
    assert_eq!(history.get_affinity(&a, &b), 0.0);
}

#[test]
fn affinity_increases_after_update() {
    let mut history = history(100);
    let (a, b) = paths("/a.jpg", "/b.jpg");
    history.update_affinity(&a, &b, None);
    assert!(history.get_affinity(&a, &b) > 0.0);
}

#[test]
fn affinity_is_commutative() {
    let mut history = history(100);
    let (a, b) = paths("/a.jpg", "/b.jpg");
    history.update_affinity(&a, &b, Some(120));
    assert_eq!(history.get_affinity(&a, &b), history.get_affinity(&b, &a));
}

#[test]
fn affinity_grows_with_repeated_pairings() {
    let mut history = history(100);
    let (a, b) = paths("/a.jpg", "/b.jpg");
    history.update_affinity(&a, &b, Some(60));
    let first = history.get_affinity(&a, &b);
    history.update_affinity(&a, &b, Some(300));
    let second = history.get_affinity(&a, &b);
    assert!(second > first, "repeated pairings should increase affinity");
}

#[test]
fn rebuild_affinity_matches_manual_updates() {
    let mut history = history(100);
    let record1 = pairing(&[("HDMI-A-1", "/a.jpg"), ("DP-1", "/b.jpg")]);
    let record2 = pairing(&[("HDMI-A-1", "/a.jpg"), ("DP-1", "/c.jpg")]);
    let record3 = pairing(&[("HDMI-A-1", "/a.jpg"), ("DP-1", "/b.jpg")]);

    history.data.records = vec![
        PairingRecord {
            wallpapers: record1,
            timestamp: 1,
            duration: Some(60),
            manual: true,
        },
        PairingRecord {
            wallpapers: record2,
            timestamp: 2,
            duration: Some(120),
            manual: false,
        },
        PairingRecord {
            wallpapers: record3,
            timestamp: 3,
            duration: Some(300),
            manual: true,
        },
    ];

    history.rebuild_affinity();

    let a = PathBuf::from("/a.jpg");
    let b = PathBuf::from("/b.jpg");
    let c = PathBuf::from("/c.jpg");
    let ab = history.get_affinity(&a, &b);
    let ac = history.get_affinity(&a, &c);

    assert!(
        ab > ac,
        "two pairings of A+B should outrank one pairing of A+C"
    );
    assert!(ab > 0.0 && ac > 0.0);
}

#[test]
fn rebuild_affinity_is_idempotent() {
    let mut history = history(100);
    history.data.records = vec![PairingRecord {
        wallpapers: pairing(&[("HDMI-A-1", "/a.jpg"), ("DP-1", "/b.jpg")]),
        timestamp: 1,
        duration: Some(90),
        manual: true,
    }];

    history.rebuild_affinity();
    let first_scores = history.data.affinity_scores.clone();
    history.rebuild_affinity();
    assert_eq!(history.data.affinity_scores.len(), first_scores.len());
    for (left, right) in history.data.affinity_scores.iter().zip(first_scores.iter()) {
        assert_eq!(left.wallpaper_a, right.wallpaper_a);
        assert_eq!(left.wallpaper_b, right.wallpaper_b);
        assert_eq!(left.pair_count, right.pair_count);
        assert!((left.score - right.score).abs() < f32::EPSILON);
        assert!((left.avg_duration_secs - right.avg_duration_secs).abs() < f32::EPSILON);
    }
}

#[test]
fn undo_not_available_initially() {
    let history = history(100);
    assert!(!history.can_undo());
    assert!(history.undo_state().is_none());
}

#[test]
fn arm_undo_makes_undo_available() {
    let mut history = history(100);
    history.arm_undo(pairing(&[("DP-1", "/old.jpg")]), 30, "Undo test");
    assert!(history.can_undo());
    assert_eq!(history.undo_message(), Some("Undo test"));
}

#[test]
fn do_undo_returns_previous_wallpapers_and_clears_state() {
    let mut history = history(100);
    let previous = pairing(&[("DP-1", "/old.jpg"), ("HDMI-A-1", "/other.jpg")]);
    history.arm_undo(previous.clone(), 30, "Undo test");
    assert_eq!(history.do_undo(), Some(previous));
    assert!(!history.can_undo());
}

#[test]
fn arm_undo_with_empty_wallpapers_disables_undo() {
    let mut history = history(100);
    history.arm_undo(HashMap::new(), 30, "No-op");
    assert!(!history.can_undo());
}

#[test]
fn arm_undo_with_zero_duration_disables_undo() {
    let mut history = history(100);
    history.arm_undo(pairing(&[("DP-1", "/old.jpg")]), 0, "No-op");
    assert!(!history.can_undo());
}

#[test]
fn prune_removes_oldest_records_when_over_limit() {
    let mut history = history(2);
    history.data.records = vec![
        PairingRecord {
            wallpapers: pairing(&[("A", "/1.jpg"), ("B", "/2.jpg")]),
            timestamp: 1,
            duration: Some(10),
            manual: true,
        },
        PairingRecord {
            wallpapers: pairing(&[("A", "/3.jpg"), ("B", "/4.jpg")]),
            timestamp: 2,
            duration: Some(10),
            manual: true,
        },
        PairingRecord {
            wallpapers: pairing(&[("A", "/5.jpg"), ("B", "/6.jpg")]),
            timestamp: 3,
            duration: Some(10),
            manual: true,
        },
    ];

    history.prune_old_records();
    assert_eq!(history.record_count(), 2);
    assert_eq!(history.data.records[0].timestamp, 2);
    assert_eq!(history.data.records[1].timestamp, 3);
}

#[test]
fn prune_removes_stale_affinity_entries() {
    let mut history = history(1);
    history.data.records = vec![PairingRecord {
        wallpapers: pairing(&[("A", "/keep-a.jpg"), ("B", "/keep-b.jpg")]),
        timestamp: 1,
        duration: Some(10),
        manual: true,
    }];
    history.data.affinity_scores = vec![
        AffinityScore {
            wallpaper_a: PathBuf::from("/keep-a.jpg"),
            wallpaper_b: PathBuf::from("/keep-b.jpg"),
            score: 0.5,
            pair_count: 1,
            avg_duration_secs: 10.0,
        },
        AffinityScore {
            wallpaper_a: PathBuf::from("/stale-a.jpg"),
            wallpaper_b: PathBuf::from("/stale-b.jpg"),
            score: 0.9,
            pair_count: 3,
            avg_duration_secs: 20.0,
        },
    ];

    history.prune_old_records();
    assert_eq!(history.affinity_count(), 1);
    assert_eq!(
        history.data.affinity_scores[0].wallpaper_a,
        PathBuf::from("/keep-a.jpg")
    );
}
