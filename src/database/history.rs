use color_eyre::eyre::{Context, Result, bail};
use sqlx::{Postgres, query_as, query_scalar};
use time::{Duration, OffsetDateTime, UtcDateTime};

use crate::database::{Database, changelog::ratio, models::MeasurementEntry};

impl Database {
    #[tracing::instrument(skip(self))]
    pub async fn get_latest(&self) -> Result<MeasurementEntry> {
        tracing::debug!("Fetching peak lazer user count measurement");
        let result = query_as!(
            MeasurementEntry,
            r#"
SELECT
    EXTRACT(EPOCH FROM inserted_at)::BIGINT AS "timestamp!",
    stable,
    lazer
FROM measurements
ORDER BY inserted_at DESC
LIMIT 1
            "#,
        )
        .fetch_one(&*self)
        .await?;
        tracing::info!(
            stable = result.stable,
            lazer = result.lazer,
            "Found user peak at {}",
            result.timestamp
        );

        Ok(result)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_user_count_peak(&self) -> Result<MeasurementEntry> {
        tracing::debug!("Fetching peak lazer user count measurement");
        let result = query_as::<_, MeasurementEntry>(
            r#"
SELECT
    EXTRACT(EPOCH FROM inserted_at)::BIGINT AS timestamp,
    stable,
    lazer
FROM measurements
ORDER BY lazer DESC, inserted_at ASC
LIMIT 1
            "#,
        )
        .fetch_one(&*self)
        .await?;
        tracing::info!(
            stable = result.stable,
            lazer = result.lazer,
            "Found user peak at {}",
            result.timestamp
        );

        Ok(result)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_user_ratio_peak(&self) -> Result<MeasurementEntry> {
        tracing::debug!("Fetching peak lazer ratio measurement");
        let result = query_as::<_, MeasurementEntry>(
            r#"
SELECT
    EXTRACT(EPOCH FROM inserted_at)::BIGINT AS timestamp,
    stable,
    lazer
FROM measurements
WHERE (stable + lazer) > 5000
ORDER BY (lazer::DOUBLE PRECISION / NULLIF((stable + lazer)::DOUBLE PRECISION, 0)) DESC,
         inserted_at ASC
LIMIT 1
            "#,
        )
        .fetch_one(&*self)
        .await?;
        tracing::info!(
            stable = result.stable,
            lazer = result.lazer,
            ratio = ratio(result.stable, result.lazer),
            "Found lazer% peak at {}",
            result.timestamp
        );

        Ok(result)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_user_highest_percentile_peak(&self) -> Result<MeasurementEntry> {
        tracing::debug!("Fetching highest user count within peak ratio percentile");
        let max_percentage: f64 = query_scalar::<Postgres, f64>(
            r#"
            SELECT MAX(lazer::DOUBLE PRECISION / NULLIF((stable + lazer)::DOUBLE PRECISION, 0))
            FROM measurements
            WHERE ((stable + lazer) > 3000)
              AND lazer > 0 AND stable > 0
            "#,
        )
        .fetch_one(&*self)
        .await?;

        tracing::debug!(max_percentage, "Found maximum lazer ratio");

        let peak: MeasurementEntry = query_as(
            r#"
            SELECT
                EXTRACT(EPOCH FROM inserted_at)::BIGINT AS timestamp,
                stable,
                lazer
            FROM measurements
            WHERE ((stable + lazer) > 3000)
              AND lazer > 0 AND stable > 0
              AND (lazer::DOUBLE PRECISION / NULLIF((stable + lazer)::DOUBLE PRECISION, 0)) >= $1
            ORDER BY lazer DESC, inserted_at ASC
            LIMIT 1
            "#,
        )
        .bind(max_percentage - 0.015)
        .fetch_one(&*self)
        .await?;

        tracing::info!(
            timestamp = peak.timestamp,
            stable = peak.stable,
            lazer = peak.lazer,
            ratio = ratio(peak.stable, peak.lazer),
            "Found percentile-adjusted lazer ratio peak"
        );

        Ok(peak)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_history_range(
        &self,
        start: time::OffsetDateTime,
        end: time::OffsetDateTime,
    ) -> Result<Vec<MeasurementEntry>> {
        tracing::debug!("Fetching measurement history range");

        let result = query_as!(
            MeasurementEntry,
            r#"
SELECT
    EXTRACT(EPOCH FROM inserted_at)::BIGINT AS "timestamp!",
    stable,
    lazer
FROM measurements
WHERE inserted_at >= $1 AND inserted_at < $2
ORDER BY inserted_at ASC
            "#,
            start.into(),
            end.into()
        )
        .fetch_all(&*self)
        .await?;

        tracing::info!(len = result.len(), "Fetched history data");
        Ok(result)
    }

    pub async fn get_past_day(&self) -> Result<Vec<MeasurementEntry>> {
        let now = OffsetDateTime::now_utc();
        let start = now.clone().saturating_sub(Duration::DAY);

        self.get_history_range(start, now)
            .await
            .wrap_err("Failed to fetch past day")
    }
}
