use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;

use crate::{
    api::models::{PointLineResponse, RatioRegressionResponse, SinglePointResponse},
    database::models::BucketSize,
    state::SharedState,
};

async fn get_current(
    State(state): State<SharedState>,
) -> Result<Json<SinglePointResponse>, StatusCode> {
    let changelog = state
        .get_latest_changelog()
        .await
        .inspect_err(|e| tracing::warn!("Error on current data: {e}"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(
        stable = changelog.stable,
        lazer = changelog.lazer,
        "Served current bar data"
    );

    Ok(Json(changelog))
}

async fn get_highest_user_count(
    State(state): State<SharedState>,
) -> Result<Json<SinglePointResponse>, StatusCode> {
    let response = state
        .get_peak_user_count()
        .await
        .inspect_err(|error| tracing::warn!(%error, "Failed to fetch peak user count from cache"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::info!("Served peak user count bar data");

    Ok(Json(response))
}

async fn get_highest_user_percentage(
    State(state): State<SharedState>,
) -> Result<Json<SinglePointResponse>, StatusCode> {
    let response = state
        .get_peak_user_ratio()
        .await
        .inspect_err(|error| tracing::warn!(%error, "Failed to fetch peak ratio from cache"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::info!("Served peak ratio bar data");

    Ok(Json(response))
}

async fn get_highest_user_count_within_85th_percentile(
    State(state): State<SharedState>,
) -> Result<Json<SinglePointResponse>, StatusCode> {
    let response = state
        .get_peak_user_percentile()
        .await
        .inspect_err(|error| tracing::warn!(%error, "Failed to fetch peak percentile from cache"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::info!("Served peak percentile bar data");

    Ok(Json(response))
}

pub async fn user_count_graph(
    State(state): State<SharedState>,
) -> Result<Json<PointLineResponse>, StatusCode> {
    let response = state
        .get_day_user_graph()
        .await
        .inspect_err(|error| tracing::warn!(%error, "Failed to fetch daily graph from cache"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::info!(points = response.timestamp.len(), "Served daily graph data");

    Ok(Json(response))
}

#[derive(Deserialize, Default)]
pub struct HistoryQuery {
    from: Option<i64>,
    to: Option<i64>,
    #[serde(default)]
    bucket_size: BucketSize,
}

pub async fn history_user_graph(
    State(state): State<SharedState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<PointLineResponse>, StatusCode> {
    let response: PointLineResponse;
    match (query.from, query.to) {
        (None, None) => {
            if let BucketSize::Day = query.bucket_size {
                response = state
                    .get_history_user_graph()
                    .await
                    .inspect_err(
                        |error| tracing::warn!(%error, "Failed to fetch history graph from cache"),
                    )
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            } else {
                response = state
                    .database()
                    .get_history(query.bucket_size)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .into();
            }
        }
        (None, Some(_)) | (Some(_), None) => return Err(StatusCode::BAD_REQUEST),
        (Some(from), Some(to)) => {
            response = state
                .database()
                .get_history_range(
                    time::OffsetDateTime::from_unix_timestamp(from)
                        .map_err(|_| StatusCode::BAD_REQUEST)?,
                    time::OffsetDateTime::from_unix_timestamp(to)
                        .map_err(|_| StatusCode::BAD_REQUEST)?,
                )
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .into()
        }
    }
    tracing::info!(
        points = response.timestamp.len(),
        "Served history graph data"
    );

    Ok(Json(response))
}

pub async fn ratio_estimate(
    State(state): State<SharedState>,
    Path(percentage): Path<f64>,
) -> Result<Json<RatioRegressionResponse>, StatusCode> {
    if !percentage.is_finite() || !(0.0..=100.0).contains(&percentage) {
        tracing::warn!(percentage, "Rejected invalid ratio estimate target");
        return Err(StatusCode::BAD_REQUEST);
    }
    let estimate = state
        .database()
        .estimate_ratio_percentage(percentage)
        .await
        .inspect_err(|error| tracing::warn!(%error, percentage, "Failed to estimate ratio target"))
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    Ok(Json(estimate.into()))
}

pub fn router() -> Router<SharedState> {
    tracing::debug!("Building bars router");
    Router::new()
        .route("/current", get(get_current))
        .route("/peak_users", get(get_highest_user_count))
        .route("/peak_ratio", get(get_highest_user_percentage))
        .route(
            "/peak_percentile",
            get(get_highest_user_count_within_85th_percentile),
        )
        .route("/charts/day", get(user_count_graph))
        .route("/charts/history", get(history_user_graph))
        .route("/charts/ratio_estimate/{percentage}", get(ratio_estimate))
}
