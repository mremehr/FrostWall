use super::*;

const STRICT_VISUAL_MIN: f32 = 0.62;
const STRICT_SEMANTIC_MIN: f32 = 0.58;
const STRICT_COMBINED_QUALITY_MIN: f32 = 0.63;
const SCREEN_CONTEXT_LOOKBACK_RECORDS: usize = 600;
const SCREEN_CONTEXT_HALF_LIFE_SECS: f32 = 7.0 * 24.0 * 3600.0;
const REPETITION_LOOKBACK_RECORDS: usize = 20;

struct MatchWeights {
    screen_context: f32,
    visual: f32,
    harmony: f32,
    tag: f32,
    semantic: f32,
    repetition_penalty: f32,
}

struct SelectedTagSets<'a> {
    tags: HashSet<&'a str>,
    style_tags: HashSet<&'a str>,
    specific_style_tags: HashSet<&'a str>,
    content_tags: HashSet<&'a str>,
}

struct CandidateTagMetrics {
    shared_tags: usize,
    content_overlap: usize,
    style_overlap: usize,
    specific_style_overlap: usize,
}

impl MatchWeights {
    fn for_context(context: &MatchContext<'_>) -> Self {
        match context.style_mode {
            PairingStyleMode::Strict => Self {
                screen_context: context.screen_context_weight * STRICT_SCREEN_CTX_SCALE,
                visual: context.visual_weight * STRICT_VISUAL_SCALE,
                harmony: context.harmony_weight * STRICT_HARMONY_SCALE,
                tag: context.tag_weight * STRICT_TAG_SCALE,
                semantic: context.semantic_weight * STRICT_SEMANTIC_SCALE,
                repetition_penalty: context.repetition_penalty_weight * STRICT_REPETITION_SCALE,
            },
            PairingStyleMode::Soft => Self {
                screen_context: context.screen_context_weight * SOFT_SCREEN_CTX_SCALE,
                visual: context.visual_weight * SOFT_VISUAL_SCALE,
                harmony: context.harmony_weight,
                tag: context.tag_weight * SOFT_TAG_SCALE,
                semantic: context.semantic_weight * SOFT_SEMANTIC_SCALE,
                repetition_penalty: context.repetition_penalty_weight,
            },
            PairingStyleMode::Off => Self {
                screen_context: context.screen_context_weight,
                visual: context.visual_weight,
                harmony: context.harmony_weight,
                tag: context.tag_weight,
                semantic: context.semantic_weight,
                repetition_penalty: context.repetition_penalty_weight,
            },
        }
    }

    fn history_scale(style_mode: PairingStyleMode) -> f32 {
        match style_mode {
            PairingStyleMode::Strict => STRICT_HISTORY_SCALE,
            PairingStyleMode::Soft => SOFT_HISTORY_SCALE,
            PairingStyleMode::Off => 1.0,
        }
    }
}

impl<'a> SelectedTagSets<'a> {
    fn from_context(context: &'a MatchContext<'a>) -> Self {
        let tags: HashSet<&str> = context.selected_tags.iter().map(String::as_str).collect();
        let style_tags: HashSet<&str> = context
            .selected_style_tags
            .iter()
            .map(String::as_str)
            .collect();
        let specific_style_tags: HashSet<&str> = style_tags
            .iter()
            .copied()
            .filter(|tag| is_specific_style_tag(tag))
            .collect();
        let content_tags: HashSet<&str> = tags
            .iter()
            .copied()
            .filter(|tag| is_content_tag(tag))
            .collect();

        Self {
            tags,
            style_tags,
            specific_style_tags,
            content_tags,
        }
    }
}

fn normalized_weights<'a>(colors: &[String], weights: &'a [f32]) -> Cow<'a, [f32]> {
    if weights.is_empty() {
        Cow::Owned(vec![1.0 / colors.len().max(1) as f32; colors.len()])
    } else {
        Cow::Borrowed(weights)
    }
}

fn candidate_tag_metrics(
    style_mode: PairingStyleMode,
    selected: &SelectedTagSets<'_>,
    candidate_tags: &[&str],
) -> CandidateTagMetrics {
    let shared_tags = candidate_tags
        .iter()
        .filter(|tag| selected.tags.contains(**tag))
        .count();
    let content_overlap = if selected.content_tags.is_empty() {
        0
    } else {
        candidate_tags
            .iter()
            .filter(|tag| selected.content_tags.contains(**tag))
            .count()
    };

    let (style_overlap, specific_style_overlap) =
        if style_mode == PairingStyleMode::Off || selected.style_tags.is_empty() {
            (0, 0)
        } else {
            let candidate_style_tags = collect_style_tags(candidate_tags.iter().copied());
            let style_overlap = candidate_style_tags
                .iter()
                .filter(|tag| selected.style_tags.contains(**tag))
                .count();
            let specific_style_overlap = candidate_style_tags
                .iter()
                .filter(|tag| selected.specific_style_tags.contains(**tag))
                .count();
            (style_overlap, specific_style_overlap)
        };

    CandidateTagMetrics {
        shared_tags,
        content_overlap,
        style_overlap,
        specific_style_overlap,
    }
}

fn semantic_similarity(
    context: &MatchContext<'_>,
    candidate: &crate::wallpaper::Wallpaper,
) -> Option<f32> {
    if let (Some(selected_embedding), Some(candidate_embedding)) =
        (context.selected_embedding, candidate.embedding.as_deref())
    {
        Some(normalize_cosine_similarity(
            selected_embedding,
            candidate_embedding,
        ))
    } else {
        None
    }
}

fn style_overlap_basis(
    selected: &SelectedTagSets<'_>,
    metrics: &CandidateTagMetrics,
) -> (usize, usize) {
    if selected.specific_style_tags.is_empty() {
        (metrics.style_overlap, selected.style_tags.len())
    } else {
        (
            metrics.specific_style_overlap,
            selected.specific_style_tags.len(),
        )
    }
}

impl PairingHistory {
    /// Get the best matching wallpaper for other screens.
    /// Returns the wallpaper with highest affinity score, or falls back to
    /// a wallpaper with similar colors if no history exists.
    pub fn get_best_match(
        &self,
        context: &MatchContext<'_>,
        available_wallpapers: &[&crate::wallpaper::Wallpaper],
    ) -> Option<PathBuf> {
        self.get_top_matches(context, available_wallpapers, 1)
            .into_iter()
            .next()
            .map(|(path, _)| path)
    }

    /// Get top N matching wallpapers for other screens.
    /// Returns wallpapers sorted by affinity score (highest first).
    pub fn get_top_matches(
        &self,
        context: &MatchContext<'_>,
        available_wallpapers: &[&crate::wallpaper::Wallpaper],
        limit: usize,
    ) -> Vec<(PathBuf, f32)> {
        if limit == 0 || available_wallpapers.is_empty() {
            return Vec::new();
        }

        let selected_weights =
            normalized_weights(context.selected_colors, context.selected_weights);
        let selected_tags = SelectedTagSets::from_context(context);
        let weights = MatchWeights::for_context(context);

        let affinity_lookup: HashMap<&Path, f32> = self
            .data
            .affinity_scores
            .iter()
            .filter_map(|score| {
                if score.wallpaper_a == context.selected_wp {
                    Some((score.wallpaper_b.as_path(), score.score))
                } else if score.wallpaper_b == context.selected_wp {
                    Some((score.wallpaper_a.as_path(), score.score))
                } else {
                    None
                }
            })
            .collect();
        let screen_context_lookup =
            self.screen_context_scores(context.selected_wp, context.target_screen);
        let history_scale = MatchWeights::history_scale(context.style_mode);

        let mut scored: Vec<(PathBuf, f32)> = available_wallpapers
            .iter()
            .filter(|wallpaper| wallpaper.path != context.selected_wp)
            .filter_map(|wallpaper| {
                let affinity = affinity_lookup
                    .get(wallpaper.path.as_path())
                    .copied()
                    .unwrap_or(0.0);
                let screen_context = screen_context_lookup
                    .get(wallpaper.path.as_path())
                    .copied()
                    .unwrap_or(0.0);
                let mut score = (affinity * weights.screen_context
                    + screen_context * weights.screen_context)
                    * history_scale;

                let mut unique_tags = HashSet::new();
                let candidate_tags: Vec<&str> = wallpaper
                    .tags
                    .iter()
                    .map(String::as_str)
                    .chain(wallpaper.auto_tags.iter().map(|tag| tag.name.as_str()))
                    .filter(|tag| unique_tags.insert(*tag))
                    .collect();
                let metrics =
                    candidate_tag_metrics(context.style_mode, &selected_tags, &candidate_tags);
                let semantic_similarity = semantic_similarity(context, wallpaper);

                if context.style_mode == PairingStyleMode::Strict {
                    if !selected_tags.style_tags.is_empty() {
                        let (overlap, basis) = style_overlap_basis(&selected_tags, &metrics);
                        if overlap == 0 {
                            return None;
                        }
                        if basis >= 2 && (overlap as f32 / basis as f32) < 0.5 {
                            return None;
                        }
                    }

                    if !selected_tags.content_tags.is_empty() {
                        if metrics.content_overlap == 0 {
                            return None;
                        }
                        if selected_tags.content_tags.len() >= 3
                            && (metrics.content_overlap as f32
                                / selected_tags.content_tags.len() as f32)
                                < 0.34
                        {
                            return None;
                        }
                    }

                    if let Some(similarity) = semantic_similarity {
                        if similarity < STRICT_SEMANTIC_MIN {
                            return None;
                        }
                    }
                }

                let candidate_weights =
                    normalized_weights(&wallpaper.colors, wallpaper.color_weights.as_slice());
                let visual_similarity = crate::utils::image_similarity_weighted(
                    context.selected_colors,
                    selected_weights.as_ref(),
                    &wallpaper.colors,
                    candidate_weights.as_ref(),
                );
                score += visual_similarity * weights.visual;

                let (harmony, strength) = crate::utils::detect_harmony(
                    context.selected_colors,
                    selected_weights.as_ref(),
                    &wallpaper.colors,
                    candidate_weights.as_ref(),
                );
                score += harmony.bonus() * strength * weights.harmony;
                score += (metrics.shared_tags as f32).min(TAG_MAX_SHARED) * weights.tag;

                match context.style_mode {
                    PairingStyleMode::Off => {}
                    PairingStyleMode::Soft => {
                        if !selected_tags.style_tags.is_empty() {
                            if metrics.style_overlap > 0 {
                                score += (metrics.style_overlap as f32).min(TAG_SOFT_STYLE_MAX)
                                    * (weights.tag * TAG_SOFT_STYLE_BONUS_MULT);
                            } else {
                                score -= weights.tag * TAG_SOFT_STYLE_PENALTY_MULT;
                            }
                        }
                        if !selected_tags.content_tags.is_empty() {
                            if metrics.content_overlap > 0 {
                                score += (metrics.content_overlap as f32).min(TAG_SOFT_CONTENT_MAX)
                                    * (weights.tag * TAG_SOFT_CONTENT_BONUS_MULT);
                            } else {
                                score -= weights.tag * TAG_SOFT_CONTENT_PENALTY_MULT;
                            }
                        }
                    }
                    PairingStyleMode::Strict => {
                        if !selected_tags.style_tags.is_empty() {
                            let (overlap, _) = style_overlap_basis(&selected_tags, &metrics);
                            if overlap > 0 {
                                score += (overlap as f32).min(TAG_STRICT_STYLE_MAX)
                                    * (weights.tag * TAG_STRICT_BONUS_MULT);
                            } else {
                                score -= weights.tag * TAG_STRICT_PENALTY_MULT;
                            }
                        }

                        if !selected_tags.content_tags.is_empty() {
                            score += (metrics.content_overlap as f32).min(TAG_STRICT_CONTENT_MAX)
                                * (weights.tag * TAG_STRICT_CONTENT_BONUS_MULT);
                        } else if selected_tags.style_tags.is_empty()
                            && visual_similarity < STRICT_VISUAL_MIN
                        {
                            return None;
                        }

                        let strict_quality = if let Some(similarity) = semantic_similarity {
                            (similarity * QUALITY_SEMANTIC_WEIGHT)
                                + (visual_similarity * QUALITY_VISUAL_WEIGHT)
                        } else {
                            visual_similarity
                        };
                        if strict_quality < STRICT_COMBINED_QUALITY_MIN {
                            return None;
                        }
                    }
                }

                if let Some(similarity) = semantic_similarity {
                    score += similarity * weights.semantic;
                }

                score -= self.recent_repetition_penalty(
                    context.target_screen,
                    &wallpaper.path,
                    weights.repetition_penalty,
                );

                Some((wallpaper.path.clone(), score))
            })
            .collect();

        if scored.len() > limit {
            let pivot = limit - 1;
            scored.select_nth_unstable_by(pivot, compare_scored_match);
            scored.truncate(limit);
        }
        scored.sort_unstable_by(compare_scored_match);
        scored
    }

    /// Build a screen-specific affinity map for selected wallpaper -> candidate on target screen.
    fn screen_context_scores(
        &self,
        selected_wp: &Path,
        target_screen: &str,
    ) -> HashMap<&Path, f32> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        let lookback = self.data.records.len().min(SCREEN_CONTEXT_LOOKBACK_RECORDS);
        let mut raw_scores: HashMap<&Path, f32> = HashMap::with_capacity(lookback);
        for record in self.data.records.iter().rev().take(lookback) {
            let Some(target_path) = record.wallpapers.get(target_screen) else {
                continue;
            };
            if target_path.as_path() == selected_wp
                || !record
                    .wallpapers
                    .values()
                    .any(|path| path.as_path() == selected_wp)
            {
                continue;
            }

            let age_secs = now.saturating_sub(record.timestamp) as f32;
            let recency = 1.0 / (1.0 + age_secs / SCREEN_CONTEXT_HALF_LIFE_SECS);
            let duration_factor = (record.duration.unwrap_or(SCREEN_CTX_DEFAULT_DURATION_SECS)
                as f32
                / SCREEN_CTX_DURATION_BASELINE_SECS)
                .clamp(SCREEN_CTX_DURATION_MIN, SCREEN_CTX_DURATION_MAX);
            let manual_factor = if record.manual {
                MANUAL_PAIRING_BOOST
            } else {
                1.0
            };
            *raw_scores.entry(target_path.as_path()).or_insert(0.0) +=
                recency * duration_factor * manual_factor;
        }

        let max_score = raw_scores.values().copied().fold(0.0, f32::max);
        if max_score > 0.0 {
            raw_scores
                .values_mut()
                .for_each(|score| *score /= max_score);
        }
        raw_scores
    }

    /// Penalize exact repetition on same target output to encourage variety.
    fn recent_repetition_penalty(&self, target_screen: &str, candidate: &Path, weight: f32) -> f32 {
        if weight <= 0.0 {
            return 0.0;
        }

        let raw_penalty = self
            .data
            .records
            .iter()
            .rev()
            .take(REPETITION_LOOKBACK_RECORDS)
            .enumerate()
            .filter_map(|(idx, record)| {
                record
                    .wallpapers
                    .get(target_screen)
                    .filter(|path| path.as_path() == candidate)
                    .map(|_| 1.0 / (idx as f32 + 1.0))
            })
            .sum::<f32>();

        (raw_penalty * REPETITION_PENALTY_SCALE * weight).min(REPETITION_PENALTY_MAX * weight)
    }
}
