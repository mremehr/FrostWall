use std::path::PathBuf;

pub(super) fn compare_scored_match(a: &(PathBuf, f32), b: &(PathBuf, f32)) -> std::cmp::Ordering {
    match b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal) {
        std::cmp::Ordering::Equal => a.0.cmp(&b.0),
        order => order,
    }
}

pub(super) fn normalize_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..len {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }

    let cosine = dot / (norm_a.sqrt() * norm_b.sqrt());
    ((cosine + 1.0) / 2.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_cosine_similarity ---

    #[test]
    fn cosine_identical_vectors_returns_one() {
        let v = vec![1.0, 2.0, 3.0];
        let result = normalize_cosine_similarity(&v, &v);
        assert!(
            (result - 1.0).abs() < 1e-5,
            "identical vectors → 1.0, got {result}"
        );
    }

    #[test]
    fn cosine_opposite_vectors_returns_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let result = normalize_cosine_similarity(&a, &b);
        assert!(result.abs() < 1e-5, "opposite vectors → 0.0, got {result}");
    }

    #[test]
    fn cosine_orthogonal_vectors_returns_half() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let result = normalize_cosine_similarity(&a, &b);
        assert!(
            (result - 0.5).abs() < 1e-5,
            "orthogonal vectors → 0.5, got {result}"
        );
    }

    #[test]
    fn cosine_empty_slice_returns_zero() {
        assert_eq!(normalize_cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn cosine_zero_vector_returns_zero() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(normalize_cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_result_clamped_to_unit_range() {
        let a = vec![1.0, 1.0];
        let b = vec![2.0, 2.0];
        let result = normalize_cosine_similarity(&a, &b);
        assert!(
            (0.0..=1.0).contains(&result),
            "result out of [0,1]: {result}"
        );
    }

    #[test]
    fn cosine_different_lengths_uses_shorter() {
        let a = vec![1.0, 0.0, 999.0]; // extra element ignored
        let b = vec![1.0, 0.0];
        let result = normalize_cosine_similarity(&a, &b);
        // a[..2] == b, so should be 1.0
        assert!((result - 1.0).abs() < 1e-5, "expected 1.0, got {result}");
    }

    // --- compare_scored_match ---

    #[test]
    fn compare_higher_score_sorts_first() {
        let a = (PathBuf::from("a.png"), 0.5_f32);
        let b = (PathBuf::from("b.png"), 0.9_f32);
        // b has higher score, so b < a in sort order (descending)
        assert_eq!(compare_scored_match(&a, &b), std::cmp::Ordering::Greater);
        assert_eq!(compare_scored_match(&b, &a), std::cmp::Ordering::Less);
    }

    #[test]
    fn compare_equal_score_breaks_tie_by_path() {
        let a = (PathBuf::from("a.png"), 0.5_f32);
        let b = (PathBuf::from("b.png"), 0.5_f32);
        assert_eq!(compare_scored_match(&a, &b), std::cmp::Ordering::Less);
        assert_eq!(compare_scored_match(&b, &a), std::cmp::Ordering::Greater);
    }

    #[test]
    fn compare_same_path_and_score_returns_equal() {
        let a = (PathBuf::from("x.png"), 0.7_f32);
        let b = (PathBuf::from("x.png"), 0.7_f32);
        assert_eq!(compare_scored_match(&a, &b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn compare_sort_descending_by_score() {
        let mut matches = [
            (PathBuf::from("c.png"), 0.3_f32),
            (PathBuf::from("a.png"), 0.9_f32),
            (PathBuf::from("b.png"), 0.6_f32),
        ];
        matches.sort_by(compare_scored_match);
        let scores: Vec<f32> = matches.iter().map(|(_, s)| *s).collect();
        assert_eq!(scores, vec![0.9, 0.6, 0.3], "expected descending order");
    }
}
