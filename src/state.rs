use color_eyre::Result;
use std::sync::Arc;
use time::{Duration, OffsetDateTime, Time};

use crate::{
    api::{
        BucketTimeRange, UserIdDistributionEntry,
        models::{PointLineResponse, ScoreAggregateResponse, SinglePointResponse},
    },
    cache::Ttl,
    database::{Database, models::BucketSize},
};

pub(crate) struct AppState {
    database: Database,
    cache: redis::Client,
}

impl AppState {
    pub async fn new_shared() -> Result<SharedState> {
        let db = Database::new(&std::env::var("DATABASE_URL")?).await?;

        let redis = redis::Client::open(std::env::var("CACHE_URL")?)?;

        let app_state = AppState {
            database: db,
            cache: redis,
        };
        Ok(Arc::new(app_state))
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub async fn cache(&self) -> redis::aio::MultiplexedConnection {
        self.cache.get_multiplexed_async_connection().await.unwrap()
    }

    pub async fn get_unique_users(
        &self,
        bucket_range: BucketTimeRange,
    ) -> Result<Vec<UserIdDistributionEntry>> {
        match bucket_range {
            BucketTimeRange::Day => self.get_daily_aggregate().await,
            BucketTimeRange::Week => self.get_weekly_aggregate().await,
            BucketTimeRange::Month => self.get_monthly_aggregate().await,
        }
    }

    pub async fn get_unique_scores(
        &self,
        bucket_range: BucketTimeRange,
    ) -> Result<Vec<UserIdDistributionEntry>> {
        match bucket_range {
            BucketTimeRange::Day => self.get_daily_scores().await,
            BucketTimeRange::Week => self.get_weekly_scores().await,
            BucketTimeRange::Month => self.get_monthly_scores().await,
        }
    }
}

fn next_1am_from(now: OffsetDateTime) -> OffsetDateTime {
    (now + Duration::days(1)).replace_time(Time::from_hms(1, 0, 0).expect("1am is a valid time"))
}

fn next_1am() -> OffsetDateTime {
    let next = next_1am_from(OffsetDateTime::now_utc());
    tracing::trace!(%next, "Setting cache TTL to next 1am UTC");
    next
}

crate::cache_entry!(
    latest_changelog,
    key = "athena:changelogs:changelog:latest",
    ty = SinglePointResponse,
    ttl = Ttl::Seconds(60),
    refresh => |server| server.database().get_latest().await?.into()
);

crate::cache_entry!(
    peak_user_count,
    key = "athena:changelogs:peak:users",
    ty = SinglePointResponse,
    ttl = Ttl::Seconds(300),
    refresh => |server| server.database().get_user_count_peak().await?.into()
);

crate::cache_entry!(
    peak_user_ratio,
    key = "athena:changelogs:peak:ratio",
    ty = SinglePointResponse,
    ttl = Ttl::Seconds(300),
    refresh => |server| server.database().get_user_ratio_peak().await?.into()
);

crate::cache_entry!(
    peak_user_percentile,
    key = "athena:changelogs:peak:percentile",
    ty = SinglePointResponse,
    ttl = Ttl::Seconds(300),
    refresh => |server| server.database().get_user_highest_percentile_peak().await?.into()
);

crate::cache_entry!(
    day_user_graph,
    key = "athena:changelogs:graph:day",
    ty = PointLineResponse,
    ttl = Ttl::Seconds(300),
    refresh => |server| server.database().get_past_day().await?.into()
);

crate::cache_entry!(
    both_clients_history_graph,
    key = "athena:changelogs:graph:history",
    ty = PointLineResponse,
    ttl = Ttl::Seconds(300),
    refresh => |server| server.database().get_lazer_history(BucketSize::Day).await?.into()
);

crate::cache_entry!(
    complete_history_graph,
    key = "athena:changelogs:graph:history:complete",
    ty = PointLineResponse,
    ttl = Ttl::Seconds(86400),
    refresh => |server| server.database().get_complete_history(BucketSize::Day).await?.into()
);

crate::cache_entry!(
    daily_aggregate,
    key = "athena:unique_users_by_id:daily",
    ty = Vec<UserIdDistributionEntry>,
    ttl = Ttl::Seconds(86400),
    refresh => |server| {
        server.database().get_unique_buckets(BucketTimeRange::Day).await?
            .into_iter()
            .map(UserIdDistributionEntry::from)
            .collect()
    }
);

crate::cache_entry!(
    weekly_aggregate,
    key = "athena:unique_users_by_id:weekly",
    ty = Vec<UserIdDistributionEntry>,
    ttl = Ttl::Seconds(86400),
    refresh => |server| {
        server.database().get_unique_buckets(BucketTimeRange::Week).await?
            .into_iter()
            .map(UserIdDistributionEntry::from)
            .collect()
    }
);

crate::cache_entry!(
    monthly_aggregate,
    key = "athena:unique_users_by_id:monthly",
    ty = Vec<UserIdDistributionEntry>,
    ttl = Ttl::Seconds(604800),
    refresh => |server| {
        server.database().get_unique_buckets(BucketTimeRange::Month).await?
            .into_iter()
            .map(UserIdDistributionEntry::from)
            .collect()
    }
);

crate::cache_entry!(
    daily_scores,
    key = "athena:unique_scores:daily",
    ty = Vec<UserIdDistributionEntry>,
    ttl = Ttl::Seconds(86400),
    refresh => |server| {
        server.database().get_bucketed_scores(BucketTimeRange::Day).await?
            .into_iter()
            .map(UserIdDistributionEntry::from)
            .collect()
    }
);

crate::cache_entry!(
    weekly_scores,
    key = "athena:unique_scores:weekly",
    ty = Vec<UserIdDistributionEntry>,
    ttl = Ttl::Seconds(86400),
    refresh => |server| {
        server.database().get_bucketed_scores(BucketTimeRange::Week).await?
            .into_iter()
            .map(UserIdDistributionEntry::from)
            .collect()
    }
);

crate::cache_entry!(
    monthly_scores,
    key = "athena:unique_scores:monthly",
    ty = Vec<UserIdDistributionEntry>,
    ttl = Ttl::Seconds(604800),
    refresh => |server| {
        server.database().get_bucketed_scores(BucketTimeRange::Month).await?
            .into_iter()
            .map(UserIdDistributionEntry::from)
            .collect()
    }
);

crate::cache_entry!(
    daily_historic_graphs,
    key = "athena:daily_graph",
    ty = Vec<ScoreAggregateResponse>,
    ttl = Ttl::Until(next_1am()),
    refresh => |server| {
        server.database().get_daily_historic_graphs().await?
            .iter()
            .map(ScoreAggregateResponse::from)
            .collect()
    }
);

pub(crate) type SharedState = Arc<AppState>;

#[cfg(test)]
mod tests {
    use time::{Date, OffsetDateTime, Time};

    use super::next_1am_from;

    #[test]
    fn test_offsets() {
        let baseline = OffsetDateTime::from_unix_timestamp(1782675578).unwrap();
        assert_eq!(
            baseline,
            OffsetDateTime::new_utc(
                Date::from_calendar_date(2026, time::Month::June, 28).unwrap(),
                Time::from_hms(19, 39, 38).unwrap()
            )
        );

        let tomorrow = baseline
            .clone()
            .replace_day(baseline.day() + 1)
            .unwrap()
            .replace_time(Time::from_hms(1, 0, 0).unwrap());

        assert_eq!(
            tomorrow,
            OffsetDateTime::new_utc(
                Date::from_calendar_date(2026, time::Month::June, 29).unwrap(),
                Time::from_hms(1, 0, 0).unwrap()
            )
        )
    }

    #[test]
    fn test_next_1am_from() {
        let baseline = OffsetDateTime::from_unix_timestamp(1782675578).unwrap();
        assert_eq!(
            next_1am_from(baseline),
            OffsetDateTime::new_utc(
                Date::from_calendar_date(2026, time::Month::June, 29).unwrap(),
                Time::from_hms(1, 0, 0).unwrap()
            )
        );
    }
}
