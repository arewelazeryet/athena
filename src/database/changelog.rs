use crate::database::{
    Database,
    models::{BucketSize, DailyEntry, RatioRegression},
};
use color_eyre::{Result, eyre::bail};
use sqlx::query_as;

impl Database {
    #[tracing::instrument(skip(self))]
    pub async fn get_history(&self, bucket_size: BucketSize) -> Result<Vec<DailyEntry>> {
        tracing::debug!("Fetching daily history rows");
        let rows = match bucket_size {
            BucketSize::Day => query_as::<_, DailyEntry>(
                r#"
                SELECT
                    -- Coalesce to remove nullchecks
                    COALESCE(EXTRACT(EPOCH FROM day_bucket)::BIGINT, 1702252800) AS date,
                    COALESCE(stable_avg, 0) AS stable,
                    COALESCE(lazer_avg, 0) AS lazer
                FROM changelog_counts_daily_aggregate
                ORDER BY day_bucket ASC
                    "#,
            ),
            BucketSize::Week => query_as::<_, DailyEntry>(
                r#"
                SELECT
                    -- Coalesce to remove nullchecks
                    COALESCE(EXTRACT(EPOCH FROM time_bucket('1 week', day_bucket))::BIGINT, 1702252800) AS date,
                    COALESCE(AVG(stable_avg)::BIGINT, 0) AS stable,
                    COALESCE(AVG(lazer_avg)::BIGINT, 0) AS lazer
                FROM changelog_counts_daily_aggregate
                GROUP BY time_bucket('1 week', day_bucket)
                ORDER BY date ASC
                    "#,
            ),
            BucketSize::Month => todo!(),
        };
        let rows = rows.fetch_all(&*self).await?;

        tracing::info!(rows = rows.len(), "Fetched daily history rows");

        Ok(rows)
    }

    #[tracing::instrument(skip(self))]
    pub async fn estimate_ratio_percentage(
        &self,
        target_percentage: f64,
    ) -> Result<RatioRegression> {
        tracing::debug!(
            target_percentage,
            "Estimating ratio target from daily history"
        );
        let entries = self.get_history(BucketSize::Day).await?;
        let regression = calculate_ratio_regression(entries, target_percentage)?;

        tracing::info!(
            target_percentage,
            estimated_timestamp = regression.estimated_timestamp,
            "Estimated ratio target from daily regression"
        );

        Ok(regression)
    }
}

const SECONDS_PER_DAY: f64 = 86_400.0;

pub fn ratio(stable: i64, lazer: i64) -> f64 {
    let (stable, lazer) = (stable as f64, lazer as f64);
    lazer / (stable + lazer)
}

fn calculate_ratio_regression(
    entries: Vec<DailyEntry>,
    target_percentage: f64,
) -> Result<RatioRegression> {
    let target_ratio = target_percentage / 100.0;

    if let Some(entry) = entries
        .iter()
        .filter(|entry| entry.stable + entry.lazer > 0)
        .find(|entry| ratio(entry.stable, entry.lazer) >= target_ratio)
    {
        tracing::info!(
            target_ratio,
            estimated_timestamp = entry.date,
            "Found observed daily ratio target crossing"
        );

        return Ok(RatioRegression {
            target_ratio,
            was_reached: true,
            estimated_timestamp: entry.date,
        });
    }

    let Some(first_entry) = entries.iter().find(|entry| entry.stable + entry.lazer > 0) else {
        bail!("Cannot estimate ratio target without daily entries containing users");
    };

    let first_timestamp = first_entry.date;
    let mut points = Vec::with_capacity(entries.len());

    for entry in entries {
        if entry.stable + entry.lazer <= 0 {
            tracing::debug!(date = entry.date, "Skipping daily entry with no users");
            continue;
        }

        points.push((
            (entry.date - first_timestamp) as f64 / SECONDS_PER_DAY,
            ratio(entry.stable, entry.lazer),
        ));
    }

    if points.len() < 2 {
        bail!("Cannot estimate ratio target with fewer than two usable daily entries");
    }

    let samples = points.len();
    let sample_count = samples as f64;
    let sum_x = points.iter().map(|(x, _)| x).sum::<f64>();
    let sum_y = points.iter().map(|(_, y)| y).sum::<f64>();
    let sum_x_squared = points.iter().map(|(x, _)| x * x).sum::<f64>();
    let sum_xy = points.iter().map(|(x, y)| x * y).sum::<f64>();
    let denominator = sample_count * sum_x_squared - sum_x * sum_x;

    if denominator.abs() <= f64::EPSILON {
        bail!("Cannot estimate ratio target because daily timestamps have no variance");
    }

    let slope_per_day = (sample_count * sum_xy - sum_x * sum_y) / denominator;
    if slope_per_day.abs() <= f64::EPSILON {
        bail!("Cannot estimate ratio target because the ratio trend is flat");
    }

    let intercept = (sum_y - slope_per_day * sum_x) / sample_count;
    let estimated_day = (target_ratio - intercept) / slope_per_day;
    let estimated_timestamp = first_timestamp as f64 + estimated_day * SECONDS_PER_DAY;

    if !estimated_timestamp.is_finite()
        || estimated_timestamp < i64::MIN as f64
        || estimated_timestamp > i64::MAX as f64
    {
        bail!("Estimated ratio target timestamp is outside the supported range");
    }
    Ok(RatioRegression {
        target_ratio,
        was_reached: false,
        estimated_timestamp: estimated_timestamp.round() as i64,
    })
}
