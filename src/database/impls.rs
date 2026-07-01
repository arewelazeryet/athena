use std::time::{SystemTime, UNIX_EPOCH};

use color_eyre::eyre::{Context, Result};
use metrics::{Unit, counter, describe_counter, describe_gauge, gauge, histogram};
use sqlx::{query, query_as, types::Json};
use time::OffsetDateTime;

use crate::database::{Database, models::DatabaseScore};

pub struct ScoreDistributionResponse {
    pub stable: i64,
    pub lazer: i64,
}

pub struct BucketedResponse {
    pub bucket_floor: String,
    pub stable: i64,
    pub lazer: i64,
    pub both: i64,
}

pub struct LatestScore {
    pub ended_at: OffsetDateTime,
    pub id: i64,
}

pub enum TimePeriod {
    Hour,
    Day,
    Week,
}

impl Database {
    /// Insert a score batch into the database
    pub async fn insert_score_batch(
        &self,
        scores: impl ExactSizeIterator<Item = &DatabaseScore>,
    ) -> Result<()> {
        let batch_length = scores.len();
        let span = tracing::info_span!(target: "insert_score_batch", "Score insertion", batch_size = batch_length);
        tracing::info!(parent: &span, "Inserting a new score batch");

        let mut trans = self.begin().await?;

        for score in scores {
            // no useful data on insertion
            let _ = query!(
                r#"
    INSERT INTO scores
    (id, user_id, ruleset_id, beatmap_id, has_replay, grade, accuracy, max_combo, total_score, classic_total_score, total_score_without_mods, is_perfect_combo, legacy_perfect, pp, legacy_total_score, ended_at, build_id, lazer, data)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
                "#,
                score.id as i64,
                score.user_id as i64,
                score.mode as i16,
                score.beatmap_id as i64,
                score.replay,
                score.grade.to_string(),
                score.accuracy,
                score.max_combo as i32,
                score.total_score as i64,
                score.classic_total_score as i64,
                score.total_score_without_mods.map(|s| s as i64),
                score.is_perfect_combo,
                score.legacy_perfect,
                score.pp as f64,
                score.legacy_total_score as i64,
                score.ended_at,
                score.build_id as i16,
                score.lazer,
                serde_json::to_value(&score.data)?,
            ).execute(&mut *trans).await.wrap_err("Failed to insert a result")?;
        }

        trans.commit().await?;
        tracing::info!(parent: &span, "Commit: inserted {batch_length} scores");

        counter!("ushio.scores_inserted_total").increment(batch_length as u64);
        histogram!("ushio.scores_inserted_latest").record(batch_length as f64);
        gauge!("ushio.last_inserted_time").set(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
        );

        Ok(())
    }

    /// Fetch score distribution for all scores in datetime range
    pub async fn get_score_distribution_in_range(
        &self,
        from: OffsetDateTime,
        to: OffsetDateTime,
    ) -> Result<ScoreDistributionResponse> {
        let result = query_as!(
            ScoreDistributionResponse,
            r#"
            SELECT
                COUNT(*) FILTER (WHERE lazer = true)  AS "lazer!",
                COUNT(*) FILTER (WHERE lazer = false) AS "stable!"
            FROM scores
            WHERE ended_at > $1 AND ended_at < $2
            "#,
            from,
            to
        )
        .fetch_one(&*self)
        .await
        .wrap_err("Failed to fetch score distribution");

        result
    }

    pub async fn get_last_inserted_score(&self) -> Result<LatestScore> {
        let result = query_as!(
            LatestScore,
            r#"SELECT id, ended_at FROM scores ORDER BY id DESC LIMIT 1"#
        )
        .fetch_one(&*self)
        .await
        .wrap_err("Failed to fetch last score");

        result
    }

    pub async fn get_unique_users(&self) -> Result<Vec<BucketedResponse>> {
        counter!("ushio.database.get_daily_unique_users.query_count").increment(1);

        let result = query_as!(BucketedResponse,
                r#"
            WITH bucketed_users AS (
                SELECT
                    user_id,
                    lazer,
                    (user_id / 2000000) * 2000000 AS bucket_floor
                FROM scores
                WHERE ended_at >= NOW() - INTERVAL '24 hours'
            ),
            user_bucket_flags AS (
                SELECT
                    user_id,
                    bucket_floor,
                    BOOL_OR(lazer = true)  AS has_lazer,
                    BOOL_OR(lazer = false) AS has_stable
                FROM bucketed_users
                GROUP BY user_id, bucket_floor
            )
            SELECT
                CONCAT(bucket_floor / 1000000, 'M - ', (bucket_floor / 1000000) + 2, 'M') AS "bucket_floor!",
                COUNT(*) FILTER (WHERE has_stable AND NOT has_lazer) AS "stable!",
                COUNT(*) FILTER (WHERE has_lazer AND NOT has_stable) AS "lazer!",
                COUNT(*) FILTER (WHERE has_lazer AND has_stable)     AS "both!"
            FROM user_bucket_flags
            GROUP BY bucket_floor
            ORDER BY bucket_floor ASC;
                "#).fetch_all(&*self).await.wrap_err("Failed to get daily unique users");

        result
    }
}
