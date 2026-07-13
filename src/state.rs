use color_eyre::Result;
use redis::{AsyncCommands, Client, JsonAsyncCommands};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use time::{OffsetDateTime, Time};

use tokio::sync::Mutex;

use crate::{
    api::{UserIdDistributionEntry, models::ScoreAggregateResponse},
    database::Database,
};

pub(crate) struct AppState {
    database: Database,
    cache: redis::aio::ConnectionManager,
}

fn parse_json_root<T: DeserializeOwned>(value: &str, key: &str) -> Result<T> {
    let mut parsed: Vec<T> = serde_json::from_str(value)?;
    parsed.pop().ok_or_else(|| {
        color_eyre::eyre::eyre!("cache entry {} was missing its JSON root value", key)
    })
}

macro_rules! cache_json_pair {
    (
        $suffix:ident,
        key = $key:expr,
        ty = $ty:ty,
        ttl = $ttl:expr,
        refresh => |$this:ident| $($refresh:tt)+
    ) => {
        pastey::paste! {
            pub async fn [<set_ $suffix>](&mut self, value: &$ty) -> Result<()> {
                let payload = serde_json::to_value(value)?;
                let _: () = self.cache_mut().json_set($key, "$", &payload).await?;
                let _: bool = self.cache_mut().expire($key, $ttl).await?;

                tracing::debug!(key = $key, ttl = $ttl, "Updated cache entry");
                Ok(())
            }

            pub async fn [<refresh_ $suffix>](&mut self) -> Result<$ty> {
                tracing::info!(key = $key, "Attempting to refresh cache entry");
                let $this = self;
                let value = { $($refresh)+ };
                $this.[<set_ $suffix>](&value).await?;
                tracing::info!(key = $key, "Refreshed cache entry");
                Ok(value)
            }

            pub async fn [<get_ $suffix>](&mut self) -> Result<$ty> {
                let ttl: i64 = self.cache_mut().ttl($key).await?;

                if ttl <= 0 {
                    tracing::debug!(key = $key, ttl, "Cache entry expired or missing");
                    return self.[<refresh_ $suffix>]().await;
                }

                let serialized: String = self.cache_mut().json_get($key, "$").await?;
                let value: $ty = parse_json_root(&serialized, $key)?;

                tracing::info!(key = $key, expires_in = ttl, "Fetched cache entry");
                Ok(value)
            }
        }
    };
    (
        $suffix:ident,
        key = $key:expr,
        ty = $ty:ty,
        ttl = $ttl:expr
    ) => {
        paste! {
            pub async fn [<set_ $suffix>](&mut self, value: &$ty) -> Result<()> {
                let payload = serde_json::to_value(value)?;
                let _: () = self.cache().json_set($key, "$", &payload).await?;
                let _: bool = self.cache().expire($key, $ttl).await?;

                tracing::debug!(key = $key, ttl = $ttl, "Updated cache entry");
                Ok(())
            }

            pub async fn [<get_ $suffix>](&mut self) -> Result<$ty> {
                let ttl: i64 = self.cache().ttl($key).await?;

                if ttl <= 0 {
                    tracing::debug!(key = $key, ttl, "Cache entry expired or missing");
                    return Err(color_eyre::eyre::eyre!(
                        "cache entry {} expired without a refresh function",
                        $key
                    ));
                }

                let serialized: String = self.cache().json_get($key, "$").await?;
                let value: $ty = parse_json_root(&serialized, $key)?;

                tracing::info!(key = $key, expires_in = ttl, "Fetched cache entry");
                Ok(value)
            }

            pub async fn [<refresh_ $suffix>](&mut self) -> Result<$ty> {
                tracing::info!(key = $key, "Attempting to refresh cache entry");
                let $this = self;
                let value = { $($refresh)+ };
                $this.[<set_ $suffix>](&value).await?;
                tracing::info!(key = $key, "Refreshed cache entry");
                Ok(value)
            }
        }
    };

}

impl AppState {
    pub async fn new_shared() -> Result<SharedState> {
        let db = Database::new(&std::env::var("DATABASE_URL")?).await?;

        let redis = redis::Client::open(std::env::var("CACHE_URL")?)?;
        let redis = redis::aio::ConnectionManager::new(redis).await?;

        let app_state = AppState {
            database: db,
            cache: redis,
        };
        Ok(Arc::new(Mutex::new(app_state)))
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.database
    }

    pub fn cache(&self) -> &redis::aio::ConnectionManager {
        &self.cache
    }
    pub fn cache_mut(&mut self) -> &mut redis::aio::ConnectionManager {
        &mut self.cache
    }

    cache_json_pair!(
        daily_aggregate,
        key = "ushio:unique_users_by_id:daily",
        ty = Vec<UserIdDistributionEntry>,
        ttl = 86400,
        refresh => |server| {
            server.database().get_daily_unique_buckets().await?.into_iter().map(UserIdDistributionEntry::from).collect()
        }
    );

    cache_json_pair!(
        weekly_aggregate,
        key = "ushio:unique_users_by_id:weekly",
        ty = Vec<UserIdDistributionEntry>,
        ttl = 86400,
        refresh => |server| {
            server.database().get_weekly_unique_buckets().await?.into_iter().map(UserIdDistributionEntry::from).collect()
        }
    );
    cache_json_pair!(
        monthly_aggregate,
        key = "ushio:unique_users_by_id:monthly",
        ty = Vec<UserIdDistributionEntry>,
        ttl = 604800,
        refresh => |server| {
            server.database().get_monthly_unique_buckets().await?.into_iter().map(UserIdDistributionEntry::from).collect()
        }
    );

    pub async fn set_daily_historic_graphs(
        &mut self,
        value: &[ScoreAggregateResponse],
    ) -> Result<()> {
        let payload = serde_json::to_value(value)?;

        let now = OffsetDateTime::now_utc();
        let tomorrow = now
            .replace_day(now.day() + 1)?
            .replace_time(Time::from_hms(1, 0, 0)?);
        let _: () = self
            .cache_mut()
            .json_set("ushio:daily_graph", "$", &payload)
            .await?;
        let _: bool = self
            .cache_mut()
            .expire_at("ushio:daily_graph", tomorrow.unix_timestamp())
            .await?;

        Ok(())
    }

    pub async fn get_daily_historic_graphs(&mut self) -> Result<Vec<ScoreAggregateResponse>> {
        let ttl: i64 = self.cache_mut().ttl("ushio:daily_graph").await?;

        if ttl <= 0 {
            tracing::debug!(key = "ushio:daily_graph", ttl, "Cache entry expired");
            let graph: Vec<_> = self
                .database()
                .get_daily_historic_graphs()
                .await?
                .iter()
                .map(|v| ScoreAggregateResponse::from(v))
                .collect();
            self.set_daily_historic_graphs(&graph).await?;
        }

        let serialized: String = self.cache_mut().json_get("ushio:daily_graph", "$").await?;
        let value: Vec<ScoreAggregateResponse> = parse_json_root(&serialized, "ushio:daily_graph")?;

        Ok(value)
    }
}

pub type SharedState = Arc<Mutex<AppState>>;

#[cfg(test)]
mod tests {
    use time::{Date, OffsetDateTime, Time};

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
}
