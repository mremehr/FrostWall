use std::process::Command;

fn frostwall() -> Command {
    Command::new(env!("CARGO_BIN_EXE_frostwall"))
}

#[test]
fn test_help_exits_zero() {
    let output = frostwall().arg("--help").output().expect("failed to run");
    assert!(output.status.success(), "frostwall --help should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Intelligent wallpaper manager"),
        "help should contain description"
    );
}

#[test]
fn test_version_exits_zero() {
    let output = frostwall()
        .arg("--version")
        .output()
        .expect("failed to run");
    assert!(
        output.status.success(),
        "frostwall --version should exit 0"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("frostwall"),
        "version output should contain crate name"
    );
}

#[test]
fn test_random_with_nonexistent_dir() {
    let output = frostwall()
        .args(["random", "-d", "/tmp/frostwall_test_nonexistent_dir_12345"])
        .output()
        .expect("failed to run");
    // Should fail gracefully, not panic
    // Exit code may be non-zero but shouldn't segfault
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "should not panic on nonexistent dir"
    );
}

#[test]
fn test_random_with_empty_dir() {
    let tmp = std::env::temp_dir().join("frostwall_integration_empty");
    std::fs::create_dir_all(&tmp).unwrap();

    let output = frostwall()
        .args(["random", "-d", tmp.to_str().unwrap()])
        .output()
        .expect("failed to run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "should not panic on empty dir"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_random_with_images() {
    let tmp = std::env::temp_dir().join("frostwall_integration_images");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Create minimal valid PNG files (10x10 red squares)
    for i in 0..3 {
        let img = image::RgbImage::from_fn(10, 10, |_, _| image::Rgb([255, 0, 0]));
        img.save(tmp.join(format!("test_{}.png", i))).unwrap();
    }

    let output = frostwall()
        .args(["random", "-d", tmp.to_str().unwrap()])
        .output()
        .expect("failed to run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "should not panic with valid images: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
