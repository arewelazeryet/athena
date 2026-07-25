mod api;
mod database;
pub mod state;

use std::{env, sync::Arc};

use axum::Router;
use color_eyre::{Result, eyre::bail};
use dotenvy::dotenv;
use metrics::{Unit, describe_gauge};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{api::aggregates, state::AppState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let _ = dotenv();
    tracing::info!("Starting up");

    let state = AppState::new_shared().await?;
    state.database().migrate().await?;

    let addr = match env::var("APP_ADDR") {
        Ok(addr) => addr,
        Err(error) => {
            tracing::debug!(%error, "APP_ADDR not set, using default address");
            "0.0.0.0:6726".to_owned()
        }
    };

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(%addr, %error, "Failed to bind HTTP listener");
            bail!("Failed to startup a TCP listener");
        }
    };
    tracing::info!(%addr, "Listening for HTTP requests");

    let app: Router = Router::new()
        .nest("/api", api::router())
        .nest("/api", aggregates::router())
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
        .with_state(Arc::clone(&state));

    describe_gauge!(
        "athena.last_inserted_time",
        Unit::Seconds,
        "Timestamp of latest insertion"
    );

    tokio::select! {
        _ = axum::serve(listener, app) => {}
    }

    Ok(())
}
