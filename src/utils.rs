mod color;
mod paths;
mod similarity;

pub use color::{color_brightness, color_similarity, detect_harmony, hex_to_rgb, ColorHarmony};

pub use paths::{
    cache_dir, display_path_name, expand_tilde, home_dir, is_image_file, picture_dir,
    project_cache_dir, project_config_dir, project_data_dir,
};

pub use similarity::{
    build_palette_profile, find_similar_wallpapers_with_profiles_iter, image_similarity_weighted,
    PaletteProfile,
};
