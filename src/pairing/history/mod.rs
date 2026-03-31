use super::scoring::{compare_scored_match, normalize_cosine_similarity};
use super::style_tags::{collect_style_tags, is_content_tag, is_specific_style_tag};
use super::{
    AffinityScore, MatchContext, PairingHistory, PairingHistoryData, PairingRecord,
    PairingStyleMode, UndoState,
};
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::utils::project_cache_dir;

mod affinity;
mod matching;
mod storage;
mod undo;

#[cfg(test)]
mod tests;

// ── Affinity base-score constants ────────────────────────────────────────────
/// Diminishing-returns threshold: ln-normalisation saturates at 10 pairings.
const AFFINITY_PAIR_COUNT_SATURATION: f32 = 10.0;
/// "Canonical" long-duration pairing: 30 min (1 800 s) maps to score 1.0.
const AFFINITY_DURATION_TARGET_SECS: f32 = 1800.0;
/// Count contributes 70 % of the base affinity score.
const AFFINITY_COUNT_WEIGHT: f32 = 0.7;
/// Duration contributes 30 % of the base affinity score.
const AFFINITY_DURATION_WEIGHT: f32 = 0.3;

// ── Strict-mode feature-weight multipliers ───────────────────────────────────
/// Strict: de-emphasise raw co-occurrence so style signals dominate.
const STRICT_SCREEN_CTX_SCALE: f32 = 0.55;
/// Strict: boost visual weight to enforce palette coherence.
const STRICT_VISUAL_SCALE: f32 = 1.20;
/// Strict: lift harmony bonus slightly for aesthetic consistency.
const STRICT_HARMONY_SCALE: f32 = 1.10;
/// Strict: amplify tag signal — style/type is the primary discriminator.
const STRICT_TAG_SCALE: f32 = 1.55;
/// Strict: heavily reward CLIP semantic similarity.
const STRICT_SEMANTIC_SCALE: f32 = 1.80;
/// Strict: small lift on repetition penalty to further encourage variety.
const STRICT_REPETITION_SCALE: f32 = 1.15;

// ── Soft-mode feature-weight multipliers ─────────────────────────────────────
/// Soft: mild reduction of screen-context history weight.
const SOFT_SCREEN_CTX_SCALE: f32 = 0.90;
/// Soft: minor visual boost without overriding history.
const SOFT_VISUAL_SCALE: f32 = 1.05;
/// Soft: slight tag boost to nudge style awareness without enforcing it.
const SOFT_TAG_SCALE: f32 = 1.15;
/// Soft: moderate semantic boost.
const SOFT_SEMANTIC_SCALE: f32 = 1.20;

// ── History scale factors ────────────────────────────────────────────────────
/// Strict: history influence cut to 15 % so style features actually dominate.
const STRICT_HISTORY_SCALE: f32 = 0.15;
/// Soft: history influence at 60 % — learned pairs still matter but softer.
const SOFT_HISTORY_SCALE: f32 = 0.6;

// ── Tag scoring constants ────────────────────────────────────────────────────
/// Maximum shared-tag count that earns a base bonus (excess ignored).
const TAG_MAX_SHARED: f32 = 3.0;
/// Soft: maximum style-tag overlap that earns a bonus.
const TAG_SOFT_STYLE_MAX: f32 = 2.0;
/// Soft: bonus multiplier applied to tag_weight per overlapping style tag.
const TAG_SOFT_STYLE_BONUS_MULT: f32 = 1.5;
/// Soft: penalty fraction of tag_weight when no style tags match.
const TAG_SOFT_STYLE_PENALTY_MULT: f32 = 1.2;
/// Soft: maximum content-tag overlap that earns a bonus.
const TAG_SOFT_CONTENT_MAX: f32 = 3.0;
/// Soft: bonus multiplier applied to tag_weight per overlapping content tag.
const TAG_SOFT_CONTENT_BONUS_MULT: f32 = 1.2;
/// Soft: penalty fraction of tag_weight when no content tags match.
const TAG_SOFT_CONTENT_PENALTY_MULT: f32 = 0.6;
/// Strict: maximum style/content overlap that earns a bonus.
const TAG_STRICT_STYLE_MAX: f32 = 2.0;
/// Strict: strong style bonus multiplier — this is the whole point of strict.
const TAG_STRICT_BONUS_MULT: f32 = 4.0;
/// Strict: heavy penalty multiplier for wrong style.
const TAG_STRICT_PENALTY_MULT: f32 = 6.0;
/// Strict: maximum content-tag overlap eligible for bonus.
const TAG_STRICT_CONTENT_MAX: f32 = 3.0;
/// Strict: content bonus multiplier applied to tag_weight.
const TAG_STRICT_CONTENT_BONUS_MULT: f32 = 2.0;

// ── Quality blend weights (strict mode) ──────────────────────────────────────
/// Semantic similarity contributes 58 % of combined quality score.
const QUALITY_SEMANTIC_WEIGHT: f32 = 0.58;
/// Visual similarity contributes 42 % of combined quality score.
const QUALITY_VISUAL_WEIGHT: f32 = 0.42;

// ── Screen-context duration constants ────────────────────────────────────────
/// Duration baseline: 15 min (900 s) maps raw contribution to 1.0.
const SCREEN_CTX_DURATION_BASELINE_SECS: f32 = 900.0;
/// Default duration (seconds) used when a record has no duration stored.
const SCREEN_CTX_DEFAULT_DURATION_SECS: u64 = 90;
/// Duration factor floor — prevents near-zero contributions from short views.
const SCREEN_CTX_DURATION_MIN: f32 = 0.35;
/// Duration factor ceiling — caps outlier marathon sessions.
const SCREEN_CTX_DURATION_MAX: f32 = 1.6;
/// Manual pairings receive a 10 % contribution boost over automatic ones.
const MANUAL_PAIRING_BOOST: f32 = 1.1;

// ── Repetition penalty constants ─────────────────────────────────────────────
/// Raw score multiplier before weight application.
const REPETITION_PENALTY_SCALE: f32 = 1.2;
/// Hard cap in absolute score units (applied before weight scaling).
const REPETITION_PENALTY_MAX: f32 = 3.0;
