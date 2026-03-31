use super::encoding::encode;
use super::{Gallery, GalleryImage};

#[test]
fn gallery_slug_matches_download_prefix() {
    assert_eq!(Gallery::Unsplash.slug(), "unsplash");
    assert_eq!(Gallery::Wallhaven.slug(), "wallhaven");
}

#[test]
fn download_filename_uses_gallery_slug_and_extension() {
    let image = GalleryImage::wallhaven(
        "abc123",
        "https://w.wallhaven.cc/full/ab/wallhaven-abc123.png",
        0,
        0,
    );
    assert_eq!(image.download_filename(), "wallhaven_abc123.png");
}

#[test]
fn download_extension_defaults_to_jpg_when_missing() {
    let image = GalleryImage::unsplash("photo", "https://images.unsplash.com/photo", 0, 0, None);
    assert_eq!(image.download_extension(), "jpg");
}

#[test]
fn with_url_preserves_metadata() {
    let image = GalleryImage::unsplash("photo", "https://a", 1920, 1080, Some("Ada".into()));
    let updated = image.with_url("https://b");

    assert_eq!(updated.url, "https://b");
    assert_eq!(updated.id, "photo");
    assert_eq!(updated.width, 1920);
    assert_eq!(updated.author.as_deref(), Some("Ada"));
    assert_eq!(updated.source, Gallery::Unsplash);
}

#[test]
fn encoding_percent_encodes_spaces_and_unicode() {
    assert_eq!(encode("blue sky"), "blue%20sky");
    assert_eq!(encode("räv"), "r%C3%A4v");
}
