pub mod aggregates;
mod models;

use crate::state::{AppState, SharedState};
use apply::Apply as _;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Debug)]
struct UserIdDistributionEntry {
    stable: u32,
    lazer: u32,
    bucket: String,
}

async fn get_daily_unique_per_client(
    State(state): State<SharedState>,
) -> Result<Json<Vec<UserIdDistributionEntry>>, StatusCode> {
    state
        .lock()
        .await
        .database()
        .get_daily_unique_users()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|bucket| UserIdDistributionEntry {
            stable: bucket.stable as u32,
            lazer: bucket.lazer as u32,
            bucket: bucket.bucket_floor,
        })
        .collect::<Vec<_>>()
        .apply(Json)
        .apply(Ok)
}

pub(crate) fn router() -> Router<SharedState> {
    Router::new().route("/distribution/daily", get(get_daily_unique_per_client))
}
