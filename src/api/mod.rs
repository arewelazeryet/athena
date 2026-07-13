pub mod aggregates;
pub mod models;

use crate::{
    database::impls::BucketedResponse,
    state::{AppState, SharedState},
};
use apply::Apply as _;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Deserialize, Serialize, Debug)]
pub struct UserIdDistributionEntry {
    stable: u32,
    lazer: u32,
    both: u32,
    bucket: u32,
}

impl From<BucketedResponse> for UserIdDistributionEntry {
    fn from(value: BucketedResponse) -> Self {
        Self {
            stable: value.stable as u32,
            lazer: value.lazer as u32,
            both: value.both as u32,
            bucket: value.bucket_floor as u32,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BucketTimeRange {
    Day,
    Week,
    Month,
    All,
}
pub struct BucketQuery(BucketTimeRange);

async fn get_daily_unique_per_client(
    State(state): State<SharedState>,
) -> Result<Json<Vec<UserIdDistributionEntry>>, StatusCode> {
    state
        .lock()
        .await
        .get_daily_aggregate()
        .await
        .inspect_err(|e| tracing::warn!("Failed to return daily unique users: {e}"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(UserIdDistributionEntry::from)
        .collect::<Vec<_>>()
        .apply(Json)
        .apply(Ok)
}
async fn get_weekly_unique(
    State(state): State<SharedState>,
) -> Result<Json<Vec<UserIdDistributionEntry>>, StatusCode> {
    state
        .lock()
        .await
        .get_weekly_aggregate()
        .await
        .inspect_err(|e| tracing::warn!("Failed to return weekly unique users: {e}"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(UserIdDistributionEntry::from)
        .collect::<Vec<_>>()
        .apply(Json)
        .apply(Ok)
}
async fn get_monthly_unique(
    State(state): State<SharedState>,
) -> Result<Json<Vec<UserIdDistributionEntry>>, StatusCode> {
    state
        .lock()
        .await
        .get_monthly_aggregate()
        .await
        .inspect_err(|e| tracing::warn!("Failed to return monthly unique users: {e}"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(UserIdDistributionEntry::from)
        .collect::<Vec<_>>()
        .apply(Json)
        .apply(Ok)
}

pub(crate) fn router() -> Router<SharedState> {
    Router::new()
        .route("/distribution/daily", get(get_daily_unique_per_client))
        .route("/distribution/weekly", get(get_weekly_unique))
        .route("/distribution/monthly", get(get_monthly_unique))
}
