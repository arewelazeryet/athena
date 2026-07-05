mod api;
mod database;
pub mod state;

use std::{env, sync::Arc, time::Duration};

use axum::Router;
use color_eyre::{Result, eyre::bail};
use dotenvy::dotenv;
use futures::{SinkExt, StreamExt};
use metrics::{Unit, describe_gauge};
use rosu_v2::model::{
    GameMode, Grade,
    mods::GameMods,
    score::{Score, ScoreStatistics},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio_tungstenite::tungstenite::{Error, protocol::Message};
use tracing_subscriber::EnvFilter;

use crate::{
    api::aggregates,
    database::{Database, impls::LatestScore, models::DatabaseScore},
    state::{AppState, SharedState},
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let _ = dotenv();

    let url = env::var("SCORES_SOCKET_URL")?;
    let state = AppState::new_shared().await?;
    let lock = state.lock().await;
    lock.database().migrate().await?;
    let data = lock.database().get_last_inserted_score().await;
    drop(lock);

    let (stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("Failed to connect");

    let (mut write, mut read) = stream.split();

    match data {
        Ok(LatestScore { id, .. }) => {
            tracing::info!("Reconnecting from id = {id}");
            write.send(Message::from(format!("{id}"))).await.unwrap()
        }
        Err(_) => {
            tracing::info!("Connecting from a fresh database");
            write.send(Message::from("connect")).await.unwrap()
        }
    }

    let clone = Arc::clone(&state);

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
        .with_state(state);

    describe_gauge!(
        "ushio.last_inserted_time",
        Unit::Seconds,
        "Timestamp of latest insertion"
    );

    tokio::select! {
        _ = process_scores(&mut read, clone) => {},
        _ = axum::serve(listener, app) => {}
    }

    Ok(())
}

async fn process_scores<S: StreamExt<Item = Result<Message, Error>> + Unpin>(
    stream: &mut S,
    database: SharedState,
) {
    // Pre-allocate the scores list, as the average batch should be around 1k scores
    let mut scores_list: Vec<DatabaseScore> = Vec::with_capacity(1000);

    while let Some(res) = stream.next().await {
        match res {
            Ok(Message::Binary(data)) => {
                let score: Score = serde_json::from_slice(&data).unwrap();

                scores_list.push(Into::<DatabaseScore>::into(score));
            }
            Ok(Message::Text(text)) => match text.as_str() {
                "start-batch" => {
                    scores_list.clear();
                    tracing::info!("batch start");
                    continue;
                }
                "end-batch" => {
                    tracing::info!("batch end, sizeof: {}", scores_list.len());
                    database
                        .lock()
                        .await
                        .database()
                        .insert_score_batch(scores_list.iter())
                        .await
                        .unwrap();
                    continue;
                }
                _ => todo!(),
            },
            _ => todo!(),
        }
    }
}
