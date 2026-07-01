use sqlx::query_as;

use crate::database::Database;
use crate::database::models::{ClientType, ScoreGameMode, ScoresAggregate};
use color_eyre::Result;
use sqlx::types::BigDecimal;
use sqlx::types::time::OffsetDateTime;

impl Database {
    pub async fn get_daily_historic_graphs(&self) -> Result<Vec<ScoresAggregate>> {
        tracing::debug!("Fetching daily historic graphs");

        let aggregates: Vec<_> = query_as!(
            ScoresAggregate,
            r#"
SELECT
    COALESCE(EXTRACT(EPOCH FROM day_bucket)::BIGINT, 0) as "day_bucket!",
    ruleset_id as "ruleset_id!",
    client_type as "client_type!",

    hll_cardinality(user_hll) as "unique_user_count!",
    hll_cardinality(beatmap_hll) as "unique_beatmap_count!",

    total_daily_scores as "total_daily_scores!",
    daily_scores_with_replays as "daily_scores_with_replays!",
    daily_perfect_combos as "daily_perfect_combos!",

    daily_min_pp as "daily_min_pp!",
    daily_max_pp as "daily_max_pp!",
    daily_sum_pp as "daily_sum_pp!",

    daily_sum_total_score as "daily_sum_total_score!",
    daily_sum_classic_total_score as "daily_sum_classic_total_score!",
    daily_sum_legacy_total_score as "daily_sum_legacy_total_score!",

    daily_max_classic_total_score as "daily_max_classic_total_score!",
    daily_max_legacy_total_score as "daily_max_legacy_total_score!",

    daily_sum_accuracy as "daily_sum_accuracy!",
    daily_peak_combo as "daily_peak_combo!"

FROM scores_daily_historic;
            "#
        )
        .fetch_all(&*self)
        .await?;
        Ok(aggregates)
    }
}
