use apply::Apply;
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};

use crate::{api::models::ScoreAggregateResponse, state::SharedState};

pub async fn get_daily_aggregate_graph(
    State(state): State<SharedState>,
) -> Result<Json<Vec<ScoreAggregateResponse>>, (StatusCode, String)> {
    tracing::trace!("Requesting daily aggregates");
    state
        .get_daily_historic_graphs()
        .await
        .inspect_err(|e| {
            tracing::warn!(
                "Failed to return daily aggregates: {:?}",
                e.chain().map(|e| e.to_string()).collect::<Vec<_>>()
            )
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .into_iter()
        .collect::<Vec<_>>()
        .apply(Json)
        .apply(Ok)
}

pub fn router() -> Router<SharedState> {
    Router::new().route("/aggregate", get(get_daily_aggregate_graph))
}
