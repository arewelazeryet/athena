use bigdecimal::ToPrimitive;
use serde::Serialize;
use time::OffsetDateTime;

use crate::database::models::{ClientType, ScoreGameMode, ScoresAggregate};

#[derive(Debug, Serialize)]
pub struct ScoreAggregateResponse {
    pub day_bucket: i64,
    pub ruleset_id: ScoreGameMode,
    pub client_type: ClientType,
    pub unique_user_count: i64,
    pub unique_beatmap_count: i64,
    pub total_daily_scores: u32,
    pub daily_scores_with_replays: u32,
    pub daily_perfect_combos: u32,
    pub daily_min_pp: f64,
    pub daily_max_pp: f64,
    pub daily_sum_pp: f64,
    pub daily_sum_total_score: bigdecimal::BigDecimal,
    pub daily_sum_classic_total_score: bigdecimal::BigDecimal,
    pub daily_sum_legacy_total_score: bigdecimal::BigDecimal,
    pub daily_max_classic_total_score: i64,
    pub daily_max_legacy_total_score: i64,
    pub daily_average_accuracy: f32,
    pub daily_peak_combo: i32,
}

impl From<ScoresAggregate> for ScoreAggregateResponse {
    fn from(value: ScoresAggregate) -> Self {
        ScoreAggregateResponse {
            day_bucket: value.day_bucket,
            ruleset_id: value.ruleset_id,
            client_type: value.client_type,
            unique_user_count: value.unique_user_count as i64,
            unique_beatmap_count: value.unique_beatmap_count as i64,
            total_daily_scores: value
                .total_daily_scores
                .clone()
                .to_u32()
                .unwrap_or_default(),
            daily_scores_with_replays: value.daily_scores_with_replays.to_u32().unwrap_or_default(),
            daily_perfect_combos: value.daily_perfect_combos.to_u32().unwrap_or_default(),
            daily_min_pp: value.daily_min_pp,
            daily_max_pp: value.daily_max_pp,
            daily_sum_pp: value.daily_sum_pp,
            daily_sum_total_score: value.daily_sum_total_score,
            daily_sum_classic_total_score: value.daily_sum_classic_total_score,
            daily_sum_legacy_total_score: value.daily_sum_legacy_total_score,
            daily_max_classic_total_score: value.daily_max_classic_total_score,
            daily_max_legacy_total_score: value.daily_max_legacy_total_score,
            daily_average_accuracy: (value.daily_sum_accuracy / value.total_daily_scores)
                .to_f32()
                .unwrap_or_default(),
            daily_peak_combo: value.daily_peak_combo,
        }
    }
}
