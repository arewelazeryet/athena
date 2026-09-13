use color_eyre::{Result, eyre::Context};
use redis::{AsyncCommands, JsonAsyncCommands};
use serde::de::DeserializeOwned;
use serde::Serialize;
use time::OffsetDateTime;

use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub enum Ttl {
    Seconds(i64),
    Until(OffsetDateTime),
    Forever,
}

pub trait Cacheable: Send + Sync {
    const KEY: &'static str;
    const CHANNEL: Option<&'static str> = None;
    type Value: Serialize + DeserializeOwned + Send + Sync;

    fn ttl() -> Ttl {
        Ttl::Forever
    }

    async fn refresh(_state: &AppState) -> Result<Self::Value> {
        Err(color_eyre::eyre::eyre!(
            "cache entry {} expired without a refresh function",
            Self::KEY
        ))
    }
}

fn parse_json_root<T: DeserializeOwned>(value: &str, key: &str) -> Result<T> {
    let mut parsed: Vec<T> = serde_json::from_str(value)?;
    parsed.pop().ok_or_else(|| {
        color_eyre::eyre::eyre!("cache entry {} was missing its JSON root value", key)
    })
}

impl AppState {
    pub async fn cache_get<T: Cacheable>(&self) -> Result<T::Value> {
        let mut con = self.cache().await;
        let ttl: i64 = con
            .ttl(T::KEY)
            .await
            .wrap_err("Failed to get TTL of cache entry")?;

        if ttl < 0 && ttl != -1 {
            tracing::debug!(key = T::KEY, ttl, "Cache entry expired or missing");
            return self.cache_refresh::<T>().await;
        }

        let serialized: String = con
            .json_get(T::KEY, "$")
            .await
            .wrap_err("Failed to read cache entry")?;
        let value = parse_json_root::<T::Value>(&serialized, T::KEY)?;

        tracing::info!(key = T::KEY, expires_in = ttl, "Fetched cache entry");
        Ok(value)
    }

    pub async fn cache_set<T: Cacheable>(&self, value: &T::Value) -> Result<()> {
        let payload = serde_json::to_value(value)?;
        let ttl = T::ttl();
        let mut con = self.cache().await;

        let _: () = con
            .json_set(T::KEY, "$", &payload)
            .await
            .wrap_err("Failed to set cache entry")?;

        match ttl {
            Ttl::Seconds(seconds) => {
                let _: bool = con
                    .expire(T::KEY, seconds)
                    .await
                    .wrap_err("Failed to set TTL on cache entry")?;
            }
            Ttl::Until(when) => {
                let is_set: bool = con
                    .expire_at(T::KEY, when.unix_timestamp())
                    .await
                    .wrap_err("Failed to set absolute expiry on cache entry")?;
                if !is_set {
                    tracing::warn!(key = T::KEY, %when, "Failed to set absolute expiry on cache entry");
                }
            }
            Ttl::Forever => {
                let _: bool = con
                    .persist(T::KEY)
                    .await
                    .wrap_err("Failed to clear TTL on cache entry")?;
            }
        }

        if let Some(channel) = T::CHANNEL {
            tracing::trace!(key = T::KEY, channel, "Cache entry has an update channel");
        }

        tracing::debug!(key = T::KEY, ttl = ?ttl, "Updated cache entry");
        Ok(())
    }

    pub async fn cache_refresh<T: Cacheable>(&self) -> Result<T::Value> {
        tracing::info!(key = T::KEY, "Attempting to refresh cache entry");
        let value = T::refresh(self).await?;
        self.cache_set::<T>(&value).await?;
        tracing::info!(key = T::KEY, "Refreshed cache entry");
        Ok(value)
    }
}

#[macro_export]
macro_rules! cache_entry {
    (
        $suffix:ident,
        key = $key:expr,
        ty = $ty:ty,
        $(ttl = $ttl:expr,)?
        $(channel = $channel:expr,)?
        $(refresh => |$server:ident| $($refresh_body:tt)+)?
    ) => {
        ::pastey::paste! {
            pub struct [< $suffix:camel >];

            impl $crate::cache::Cacheable for [< $suffix:camel >] {
                const KEY: &'static str = $key;
                $(const CHANNEL: Option<&'static str> = Some($channel);)?
                type Value = $ty;

                $(fn ttl() -> $crate::cache::Ttl { $ttl })?

                $(
                    async fn refresh(
                        $server: &$crate::state::AppState,
                    ) -> ::color_eyre::Result<Self::Value> {
                        let value: Self::Value = { $($refresh_body)+ };
                        Ok(value)
                    }
                )?
            }

            impl $crate::state::AppState {
                pub async fn [< get_ $suffix >](&self) -> ::color_eyre::Result<$ty> {
                    self.cache_get::<[< $suffix:camel >]>().await
                }
            }
        }
    };
}