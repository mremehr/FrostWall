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
