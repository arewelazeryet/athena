use apply::Apply;
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};

use crate::{api::models::ScoreAggregateResponse, state::SharedState};

pub async fn get_daily_aggregate_graph(
    State(state): State<SharedState>,
) -> Result<Json<Vec<ScoreAggregateResponse>>, StatusCode> {
    tracing::trace!("Requesting daily aggregates");
    state
        .lock()
        .await
        .database()
        .get_daily_historic_graphs()
        .await
        .inspect_err(|e| tracing::warn!("Failed to return daily historic graphs: {e}"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .iter()
        .map(|s| s.into())
        .collect::<Vec<_>>()
        .apply(Json)
        .apply(Ok)
}

pub fn router() -> Router<SharedState> {
    Router::new().route("/aggregate", get(get_daily_aggregate_graph))
}
