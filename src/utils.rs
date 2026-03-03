mod color;
mod paths;
mod similarity;

pub use color::{color_brightness, color_similarity, detect_harmony, hex_to_rgb, ColorHarmony};
#[cfg(test)]
pub(crate) use color::{color_saturation, delta_e_2000, hex_to_hsl, hex_to_lab};

pub use paths::{expand_tilde, is_image_file};

pub use similarity::{
    build_palette_profile, find_similar_wallpapers_with_profiles_iter, image_similarity_weighted,
    PaletteProfile,
};
#[cfg(test)]
pub(crate) use similarity::{
    find_similar_wallpapers_iter, image_similarity, palette_similarity_weighted,
};
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // --- hex_to_rgb ---

    #[test]
    fn test_hex_to_rgb_with_hash() {
        assert_eq!(hex_to_rgb("#FF0000"), Some((255, 0, 0)));
        assert_eq!(hex_to_rgb("#00FF00"), Some((0, 255, 0)));
        assert_eq!(hex_to_rgb("#0000FF"), Some((0, 0, 255)));
        assert_eq!(hex_to_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(hex_to_rgb("#FFFFFF"), Some((255, 255, 255)));
    }

    #[test]
    fn test_hex_to_rgb_without_hash() {
        assert_eq!(hex_to_rgb("FF0000"), Some((255, 0, 0)));
        assert_eq!(hex_to_rgb("00ff00"), Some((0, 255, 0)));
    }

    #[test]
    fn test_hex_to_rgb_lowercase() {
        assert_eq!(hex_to_rgb("#ff8040"), Some((255, 128, 64)));
    }

    #[test]
    fn test_hex_to_rgb_invalid() {
        assert_eq!(hex_to_rgb("#FFF"), None); // too short
        assert_eq!(hex_to_rgb("#GGGGGG"), None); // invalid hex chars
        assert_eq!(hex_to_rgb(""), None); // empty
        assert_eq!(hex_to_rgb("#FF00FF00"), None); // too long
    }

    // --- hex_to_lab ---

    #[test]
    fn test_hex_to_lab_white() {
        let lab = hex_to_lab("#FFFFFF").unwrap();
        assert!(
            (lab.l - 100.0).abs() < 1.0,
            "White L should be ~100, got {}",
            lab.l
        );
        assert!(lab.a.abs() < 1.0, "White a should be ~0, got {}", lab.a);
        assert!(lab.b.abs() < 1.0, "White b should be ~0, got {}", lab.b);
    }

    #[test]
    fn test_hex_to_lab_black() {
        let lab = hex_to_lab("#000000").unwrap();
        assert!(lab.l.abs() < 1.0, "Black L should be ~0, got {}", lab.l);
    }

    #[test]
    fn test_hex_to_lab_invalid() {
        assert!(hex_to_lab("#GGG").is_none());
    }

    // --- hex_to_hsl ---

    #[test]
    fn test_hex_to_hsl_red() {
        let (h, s, l) = hex_to_hsl("#FF0000").unwrap();
        assert!(
            h.abs() < 1.0 || (h - 360.0).abs() < 1.0,
            "Red hue should be ~0, got {}",
            h
        );
        assert!(
            (s - 1.0).abs() < 0.01,
            "Red saturation should be 1.0, got {}",
            s
        );
        assert!(
            (l - 0.5).abs() < 0.01,
            "Red lightness should be 0.5, got {}",
            l
        );
    }

    #[test]
    fn test_hex_to_hsl_gray() {
        let (h, s, l) = hex_to_hsl("#808080").unwrap();
        assert!(
            (h - 0.0).abs() < 0.01,
            "Gray hue should be 0 (achromatic), got {}",
            h
        );
        assert!(
            (s - 0.0).abs() < 0.01,
            "Gray saturation should be 0, got {}",
            s
        );
        assert!(
            (l - 0.502).abs() < 0.02,
            "Gray lightness should be ~0.5, got {}",
            l
        );
    }

    #[test]
    fn test_hex_to_hsl_blue() {
        let (h, _s, _l) = hex_to_hsl("#0000FF").unwrap();
        assert!(
            (h - 240.0).abs() < 1.0,
            "Blue hue should be ~240, got {}",
            h
        );
    }

    #[test]
    fn test_hex_to_hsl_invalid() {
        assert!(hex_to_hsl("invalid").is_none());
    }

    // --- delta_e_2000 ---

    #[test]
    fn test_delta_e_2000_identical() {
        let lab = hex_to_lab("#FF0000").unwrap();
        let d = delta_e_2000(&lab, &lab);
        assert!(
            d.abs() < 0.001,
            "Identical colors should have delta_e ~0, got {}",
            d
        );
    }

    #[test]
    fn test_delta_e_2000_black_white() {
        let black = hex_to_lab("#000000").unwrap();
        let white = hex_to_lab("#FFFFFF").unwrap();
        let d = delta_e_2000(&black, &white);
        assert!(
            d > 50.0,
            "Black vs white should have large delta_e, got {}",
            d
        );
    }

    #[test]
    fn test_delta_e_2000_similar_colors() {
        let c1 = hex_to_lab("#FF0000").unwrap();
        let c2 = hex_to_lab("#FE0000").unwrap();
        let d = delta_e_2000(&c1, &c2);
        assert!(
            d < 2.0,
            "Very similar reds should have small delta_e, got {}",
            d
        );
    }

    #[test]
    fn test_delta_e_2000_symmetry() {
        let c1 = hex_to_lab("#FF0000").unwrap();
        let c2 = hex_to_lab("#00FF00").unwrap();
        let d1 = delta_e_2000(&c1, &c2);
        let d2 = delta_e_2000(&c2, &c1);
        assert!(
            (d1 - d2).abs() < 0.001,
            "delta_e should be symmetric: {} vs {}",
            d1,
            d2
        );
    }

    // --- detect_harmony ---

    #[test]
    fn test_detect_harmony_analogous() {
        // Red and orange-red should be analogous (close hues)
        let colors1 = vec!["#FF0000".into()];
        let colors2 = vec!["#FF3300".into()];
        let weights = vec![1.0];
        let (harmony, strength) = detect_harmony(&colors1, &weights, &colors2, &weights);
        assert_eq!(harmony, ColorHarmony::Analogous);
        assert!(
            strength > 0.0,
            "Strength should be positive, got {}",
            strength
        );
    }

    #[test]
    fn test_detect_harmony_complementary() {
        // Red and cyan should be complementary (180 degrees apart)
        let colors1 = vec!["#FF0000".into()];
        let colors2 = vec!["#00FFFF".into()];
        let weights = vec![1.0];
        let (harmony, _) = detect_harmony(&colors1, &weights, &colors2, &weights);
        assert_eq!(harmony, ColorHarmony::Complementary);
    }

    #[test]
    fn test_detect_harmony_empty() {
        let empty: Vec<String> = vec![];
        let colors = vec!["#FF0000".into()];
        let weights = vec![1.0];
        let (harmony, strength) = detect_harmony(&empty, &[], &colors, &weights);
        assert_eq!(harmony, ColorHarmony::None);
        assert!((strength - 0.0).abs() < 0.001);
    }

    // --- color_similarity ---

    #[test]
    fn test_color_similarity_identical() {
        let sim = color_similarity("#FF0000", "#FF0000");
        assert!(
            (sim - 1.0).abs() < 0.01,
            "Identical colors should have similarity ~1.0, got {}",
            sim
        );
    }

    #[test]
    fn test_color_similarity_very_different() {
        let sim = color_similarity("#000000", "#FFFFFF");
        assert!(
            sim < 0.5,
            "Black vs white should have low similarity, got {}",
            sim
        );
    }

    #[test]
    fn test_color_similarity_invalid() {
        let sim = color_similarity("invalid", "#FF0000");
        assert!((sim - 0.0).abs() < 0.001, "Invalid color should return 0.0");
    }

    // --- palette_similarity_weighted ---

    #[test]
    fn test_palette_similarity_weighted_same() {
        let colors = vec!["#FF0000".into(), "#00FF00".into()];
        let weights = vec![0.5, 0.5];
        let sim = palette_similarity_weighted(&colors, &weights, &colors, &weights);
        assert!(
            sim > 0.9,
            "Same palette should have high similarity, got {}",
            sim
        );
    }

    #[test]
    fn test_palette_similarity_weighted_empty() {
        let empty: Vec<String> = vec![];
        let colors = vec!["#FF0000".into()];
        assert_eq!(
            palette_similarity_weighted(&empty, &[], &colors, &[1.0]),
            0.0
        );
        assert_eq!(
            palette_similarity_weighted(&colors, &[1.0], &empty, &[]),
            0.0
        );
    }

    // --- color_brightness ---

    #[test]
    fn test_color_brightness_white() {
        let b = color_brightness("#FFFFFF");
        assert!(
            (b - 1.0).abs() < 0.01,
            "White brightness should be ~1.0, got {}",
            b
        );
    }

    #[test]
    fn test_color_brightness_black() {
        let b = color_brightness("#000000");
        assert!(b.abs() < 0.01, "Black brightness should be ~0.0, got {}", b);
    }

    #[test]
    fn test_color_brightness_invalid() {
        let b = color_brightness("invalid");
        assert!(
            (b - 0.5).abs() < 0.01,
            "Invalid color brightness should default to 0.5"
        );
    }

    // --- color_saturation ---

    #[test]
    fn test_color_saturation_pure_red() {
        let s = color_saturation("#FF0000");
        assert!(
            (s - 1.0).abs() < 0.01,
            "Pure red saturation should be 1.0, got {}",
            s
        );
    }

    #[test]
    fn test_color_saturation_gray() {
        let s = color_saturation("#808080");
        assert!(s.abs() < 0.01, "Gray saturation should be ~0.0, got {}", s);
    }

    #[test]
    fn test_color_saturation_white() {
        let s = color_saturation("#FFFFFF");
        assert!(s.abs() < 0.01, "White saturation should be 0.0, got {}", s);
    }

    // --- is_image_file ---

    #[test]
    fn test_is_image_file_supported() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.jpeg")));
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("photo.webp")));
        assert!(is_image_file(Path::new("photo.bmp")));
        assert!(is_image_file(Path::new("photo.gif")));
    }

    #[test]
    fn test_is_image_file_case_insensitive() {
        assert!(is_image_file(Path::new("photo.JPG")));
        assert!(is_image_file(Path::new("photo.PNG")));
        assert!(is_image_file(Path::new("photo.WebP")));
    }

    #[test]
    fn test_is_image_file_unsupported() {
        assert!(!is_image_file(Path::new("document.txt")));
        assert!(!is_image_file(Path::new("video.mp4")));
        assert!(!is_image_file(Path::new("noextension")));
        assert!(!is_image_file(Path::new(".hidden")));
    }

    // --- expand_tilde ---

    #[test]
    fn test_expand_tilde_with_tilde() {
        let expanded = expand_tilde("~/documents/test.png");
        let expanded_str = expanded.to_string_lossy();
        assert!(
            !expanded_str.starts_with("~/"),
            "Should expand ~, got {}",
            expanded_str
        );
        assert!(expanded_str.ends_with("documents/test.png"));
    }

    #[test]
    fn test_expand_tilde_absolute_path() {
        let path = "/absolute/path/file.png";
        let expanded = expand_tilde(path);
        assert_eq!(
            expanded.to_string_lossy(),
            path,
            "Absolute path should be unchanged"
        );
    }

    // --- image_similarity ---

    #[test]
    fn test_image_similarity_identical() {
        let colors = vec!["#FF0000".into(), "#00FF00".into(), "#0000FF".into()];
        let sim = image_similarity(&colors, &colors);
        assert!(
            sim > 0.9,
            "Identical palettes should have high image similarity, got {}",
            sim
        );
    }

    #[test]
    fn test_image_similarity_empty() {
        let empty: Vec<String> = vec![];
        let colors = vec!["#FF0000".into()];
        assert_eq!(image_similarity(&empty, &colors), 0.0);
    }

    // --- ColorHarmony ---

    #[test]
    fn test_color_harmony_bonus() {
        assert!(ColorHarmony::Analogous.bonus() > ColorHarmony::None.bonus());
        assert!(ColorHarmony::Complementary.bonus() > ColorHarmony::None.bonus());
        assert_eq!(ColorHarmony::None.bonus(), 0.0);
    }

    #[test]
    fn test_color_harmony_name() {
        assert_eq!(ColorHarmony::Analogous.name(), "Analogous");
        assert_eq!(ColorHarmony::Complementary.name(), "Complementary");
        assert_eq!(ColorHarmony::Triadic.name(), "Triadic");
        assert_eq!(
            ColorHarmony::SplitComplementary.name(),
            "Split-Complementary"
        );
        assert_eq!(ColorHarmony::None.name(), "None");
    }

    // --- find_similar_wallpapers ---

    #[test]
    fn test_find_similar_wallpapers_returns_sorted() {
        let target = vec!["#FF0000".into()];
        let c0 = [String::from("#0000FF")];
        let c1 = [String::from("#FF0000")];
        let c2 = [String::from("#FF1100")];
        let candidates: Vec<(usize, &[String])> = vec![(0, &c0), (1, &c1), (2, &c2)];
        let results =
            find_similar_wallpapers_iter(&target, candidates.iter().map(|(idx, c)| (*idx, *c)), 3);
        assert!(!results.is_empty());
        // Most similar (index 1, identical) should be first
        assert_eq!(results[0].1, 1, "Identical color should be best match");
        // Scores should be descending
        for w in results.windows(2) {
            assert!(
                w[0].0 >= w[1].0,
                "Results should be sorted by similarity descending"
            );
        }
    }

    #[test]
    #[ignore]
    fn bench_find_similar_wallpapers_ab_10k() {
        fn pseudo_hex(seed: usize, offset: usize) -> String {
            let r = ((seed.wrapping_mul(37) + offset.wrapping_mul(53)) & 0xff) as u8;
            let g = ((seed.wrapping_mul(73) + offset.wrapping_mul(29)) & 0xff) as u8;
            let b = ((seed.wrapping_mul(19) + offset.wrapping_mul(97)) & 0xff) as u8;
            format!("#{r:02X}{g:02X}{b:02X}")
        }

        fn baseline_find_similar(
            target_colors: &[String],
            all_wallpapers: &[(usize, &[String])],
            limit: usize,
        ) -> Vec<(f32, usize)> {
            if limit == 0 || all_wallpapers.is_empty() {
                return Vec::new();
            }

            let mut similarities: Vec<(f32, usize)> = all_wallpapers
                .iter()
                .map(|(idx, colors)| (image_similarity(target_colors, colors), *idx))
                .collect();

            if similarities.len() > limit {
                let pivot = limit - 1;
                similarities.select_nth_unstable_by(pivot, |a, b| {
                    b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
                });
                similarities.truncate(limit);
            }

            similarities.sort_unstable_by(|a, b| {
                b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
            });
            similarities
        }

        const CANDIDATES: usize = 10_000;
        const COLORS_PER_PALETTE: usize = 6;
        const LIMIT: usize = 20;

        let target: Vec<String> = (0..COLORS_PER_PALETTE).map(|i| pseudo_hex(42, i)).collect();
        let candidates: Vec<Vec<String>> = (0..CANDIDATES)
            .map(|idx| {
                (0..COLORS_PER_PALETTE)
                    .map(|j| pseudo_hex(idx + 1, j + idx))
                    .collect()
            })
            .collect();

        let candidate_refs: Vec<(usize, &[String])> = candidates
            .iter()
            .enumerate()
            .map(|(idx, c)| (idx, c.as_slice()))
            .collect();
        let target_profile = build_palette_profile(&target, &[]);
        let candidate_profiles: Vec<PaletteProfile> = candidates
            .iter()
            .map(|c| build_palette_profile(c, &[]))
            .collect();

        let baseline_start = std::time::Instant::now();
        let baseline_results = baseline_find_similar(&target, &candidate_refs, LIMIT);
        let baseline_elapsed = baseline_start.elapsed();

        let optimized_start = std::time::Instant::now();
        let optimized_results = find_similar_wallpapers_with_profiles_iter(
            &target,
            &target_profile,
            candidates
                .iter()
                .enumerate()
                .map(|(idx, c)| (idx, c.as_slice(), &candidate_profiles[idx])),
            LIMIT,
        );
        let optimized_elapsed = optimized_start.elapsed();

        eprintln!(
            "bench_find_similar_wallpapers_ab_10k: baseline={:?}, optimized={:?}, speedup={:.2}x",
            baseline_elapsed,
            optimized_elapsed,
            baseline_elapsed.as_secs_f64() / optimized_elapsed.as_secs_f64()
        );

        assert_eq!(baseline_results.len(), LIMIT);
        assert_eq!(optimized_results.len(), LIMIT);
    }
}
