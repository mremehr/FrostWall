use crate::screen::AspectCategory;
use clap::ValueEnum;

const NATIVE_BUCKETS: [RenameBucket; 4] = [
    RenameBucket::Ultrawide,
    RenameBucket::Landscape,
    RenameBucket::Portrait,
    RenameBucket::Square,
];
const LEGACY_BUCKETS: [RenameBucket; 3] = [
    RenameBucket::Widescreen,
    RenameBucket::Portrait,
    RenameBucket::Square,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum RenameScheme {
    #[default]
    Native,
    Legacy,
}

#[derive(Debug, Clone, Copy)]
pub struct RenameOptions {
    pub dry_run: bool,
    pub compact: bool,
    pub warn_content_dupes: bool,
    pub scheme: RenameScheme,
}

impl Default for RenameOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            compact: false,
            warn_content_dupes: false,
            scheme: RenameScheme::Native,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum RenameBucket {
    Ultrawide,
    Landscape,
    Portrait,
    Square,
    Widescreen,
}

impl RenameBucket {
    pub(super) fn prefix(self) -> &'static str {
        match self {
            RenameBucket::Ultrawide => "ultrawide",
            RenameBucket::Landscape => "landscape",
            RenameBucket::Portrait => "portrait",
            RenameBucket::Square => "square",
            RenameBucket::Widescreen => "widescreen",
        }
    }
}

impl RenameScheme {
    pub(super) fn supported_buckets(self) -> &'static [RenameBucket] {
        match self {
            RenameScheme::Native => &NATIVE_BUCKETS,
            RenameScheme::Legacy => &LEGACY_BUCKETS,
        }
    }

    pub(super) fn bucket_for_aspect(self, aspect: AspectCategory) -> RenameBucket {
        match self {
            RenameScheme::Native => match aspect {
                AspectCategory::Ultrawide => RenameBucket::Ultrawide,
                AspectCategory::Landscape => RenameBucket::Landscape,
                AspectCategory::Portrait => RenameBucket::Portrait,
                AspectCategory::Square => RenameBucket::Square,
            },
            RenameScheme::Legacy => match aspect {
                AspectCategory::Portrait => RenameBucket::Portrait,
                AspectCategory::Square => RenameBucket::Square,
                AspectCategory::Ultrawide | AspectCategory::Landscape => RenameBucket::Widescreen,
            },
        }
    }
}
