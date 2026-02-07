use criterion::{black_box, criterion_group, criterion_main, Criterion};
use palette::Lab;

// Re-implement the functions here since they're in a binary crate
fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

fn hex_to_lab(hex: &str) -> Option<Lab> {
    use palette::{IntoColor, Srgb};
    let (r, g, b) = hex_to_rgb(hex)?;
    let rgb = Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    Some(rgb.into_color())
}

fn hex_to_hsl(hex: &str) -> Option<(f32, f32, f32)> {
    let (r, g, b) = hex_to_rgb(hex)?;
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return Some((0.0, 0.0, l));
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    Some((h * 60.0, s, l))
}

fn delta_e_2000(lab1: &Lab, lab2: &Lab) -> f32 {
    use std::f32::consts::PI;
    let l_bar = (lab1.l + lab2.l) / 2.0;
    let c1 = (lab1.a * lab1.a + lab1.b * lab1.b).sqrt();
    let c2 = (lab2.a * lab2.a + lab2.b * lab2.b).sqrt();
    let c_bar = (c1 + c2) / 2.0;
    let c_bar_7 = c_bar.powi(7);
    let g = 0.5 * (1.0 - (c_bar_7 / (c_bar_7 + 25.0_f32.powi(7))).sqrt());
    let a1p = lab1.a * (1.0 + g);
    let a2p = lab2.a * (1.0 + g);
    let c1p = (a1p * a1p + lab1.b * lab1.b).sqrt();
    let c2p = (a2p * a2p + lab2.b * lab2.b).sqrt();
    let c_bar_p = (c1p + c2p) / 2.0;
    let h1p = lab1.b.atan2(a1p).to_degrees().rem_euclid(360.0);
    let h2p = lab2.b.atan2(a2p).to_degrees().rem_euclid(360.0);
    let dh = if (h1p - h2p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p <= h1p {
        h2p - h1p + 360.0
    } else {
        h2p - h1p - 360.0
    };
    let dl = lab2.l - lab1.l;
    let dc = c2p - c1p;
    let dh_rad = 2.0 * (c1p * c2p).sqrt() * (dh * PI / 360.0).sin();
    let h_bar_p = if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * ((h_bar_p - 30.0) * PI / 180.0).cos()
        + 0.24 * ((2.0 * h_bar_p) * PI / 180.0).cos()
        + 0.32 * ((3.0 * h_bar_p + 6.0) * PI / 180.0).cos()
        - 0.20 * ((4.0 * h_bar_p - 63.0) * PI / 180.0).cos();
    let sl = 1.0 + 0.015 * (l_bar - 50.0).powi(2) / (20.0 + (l_bar - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * c_bar_p;
    let sh = 1.0 + 0.015 * c_bar_p * t;
    let c_bar_p_7 = c_bar_p.powi(7);
    let rt = -2.0
        * (c_bar_p_7 / (c_bar_p_7 + 25.0_f32.powi(7))).sqrt()
        * (60.0 * (-((h_bar_p - 275.0) / 25.0).powi(2)).exp() * PI / 180.0).sin();
    let term_l = dl / sl;
    let term_c = dc / sc;
    let term_h = dh_rad / sh;
    (term_l * term_l + term_c * term_c + term_h * term_h + rt * term_c * term_h).sqrt()
}

fn color_similarity(hex1: &str, hex2: &str) -> f32 {
    match (hex_to_lab(hex1), hex_to_lab(hex2)) {
        (Some(lab1), Some(lab2)) => {
            let de = delta_e_2000(&lab1, &lab2);
            (1.0 - de / 100.0).max(0.0)
        }
        _ => 0.0,
    }
}

fn detect_harmony(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> f32 {
    let mut total_strength = 0.0f32;
    let mut total_weight = 0.0f32;
    for (i, c1) in colors1.iter().enumerate() {
        let w1 = weights1.get(i).copied().unwrap_or(1.0);
        if let Some(hsl1) = hex_to_hsl(c1) {
            for (j, c2) in colors2.iter().enumerate() {
                let w2 = weights2.get(j).copied().unwrap_or(1.0);
                if let Some(hsl2) = hex_to_hsl(c2) {
                    let diff = (hsl1.0 - hsl2.0).abs() % 360.0;
                    let diff = diff.min(360.0 - diff);
                    let weight = w1 * w2;
                    total_strength += (1.0 - diff / 180.0) * weight;
                    total_weight += weight;
                }
            }
        }
    }
    if total_weight > 0.0 {
        total_strength / total_weight
    } else {
        0.0
    }
}

fn bench_hex_to_rgb(c: &mut Criterion) {
    c.bench_function("hex_to_rgb", |b| {
        b.iter(|| hex_to_rgb(black_box("#FF5733")))
    });
}

fn bench_hex_to_lab(c: &mut Criterion) {
    c.bench_function("hex_to_lab", |b| {
        b.iter(|| hex_to_lab(black_box("#FF5733")))
    });
}

fn bench_hex_to_hsl(c: &mut Criterion) {
    c.bench_function("hex_to_hsl", |b| {
        b.iter(|| hex_to_hsl(black_box("#FF5733")))
    });
}

fn bench_delta_e_2000(c: &mut Criterion) {
    let lab1 = hex_to_lab("#FF5733").unwrap();
    let lab2 = hex_to_lab("#3357FF").unwrap();
    c.bench_function("delta_e_2000", |b| {
        b.iter(|| delta_e_2000(black_box(&lab1), black_box(&lab2)))
    });
}

fn bench_color_similarity(c: &mut Criterion) {
    c.bench_function("color_similarity", |b| {
        b.iter(|| color_similarity(black_box("#FF5733"), black_box("#3357FF")))
    });
}

fn bench_detect_harmony(c: &mut Criterion) {
    let colors1: Vec<String> = vec!["#FF5733".into(), "#33FF57".into(), "#3357FF".into()];
    let weights1 = vec![0.5, 0.3, 0.2];
    let colors2: Vec<String> = vec!["#FF3357".into(), "#57FF33".into(), "#5733FF".into()];
    let weights2 = vec![0.4, 0.35, 0.25];
    c.bench_function("detect_harmony_3x3", |b| {
        b.iter(|| {
            detect_harmony(
                black_box(&colors1),
                black_box(&weights1),
                black_box(&colors2),
                black_box(&weights2),
            )
        })
    });
}

criterion_group!(
    benches,
    bench_hex_to_rgb,
    bench_hex_to_lab,
    bench_hex_to_hsl,
    bench_delta_e_2000,
    bench_color_similarity,
    bench_detect_harmony,
);
criterion_main!(benches);
