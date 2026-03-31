mod layout;
mod render;

pub(super) use render::{draw_carousel, draw_carousel_single};

const THUMBNAIL_GAP: u16 = 2;
const DEFAULT_TERMINAL_CELL_ASPECT: f32 = 2.0;
const MIN_TERMINAL_CELL_ASPECT: f32 = 1.2;
const MAX_TERMINAL_CELL_ASPECT: f32 = 3.0;
const MIN_THUMB_CONTENT_HEIGHT: u16 = 8;
const LANDSCAPE_RATIO: f32 = 16.0 / 9.0;
const MIN_SLOT_WIDTH: u16 = 24;
const MAX_CAROUSEL_VISIBLE: usize = 13; // ~338 terminal columns needed at MIN_SLOT_WIDTH
const MAX_SLOT_WIDTH: u16 = 280;
const MAX_SELECTED_SLOT_WIDTH: u16 = 360;
const SELECTED_WIDTH_BOOST: f32 = 1.25;
const SELECTED_ULTRAWIDE_BOOST: f32 = 1.12;
