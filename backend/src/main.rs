use std::{collections::HashMap, sync::{Arc, Mutex}};
use tracing::info;

mod llm;
mod types;
mod game;
mod handlers;
mod websocket;
mod utils;

use types::AppState;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    
    tracing_subscriber::fmt::init();
    
    let app_state = Arc::new(Mutex::new(AppState {
        rooms: HashMap::new(),
    }));
    
    let app = axum::Router::new()
        .route("/api/create-room", axum::routing::post(handlers::create_room))
        .route("/api/join-room", axum::routing::post(handlers::join_room))
        .route("/api/player-ready-to-start", axum::routing::post(handlers::player_ready_to_start))
        .route("/api/submit-answer", axum::routing::post(handlers::submit_answer))
        .route("/api/ready-next", axum::routing::post(handlers::ready_for_next))
        .route("/api/generate-trivia", axum::routing::get(handlers::generate_trivia))
        .route("/ws", axum::routing::get(websocket::websocket_handler))
        .route("/api/room/:room_name/players", axum::routing::get(handlers::get_players_in_room))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(app_state);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    info!("Server running on http://0.0.0.0:3001");
    
    axum::serve(listener, app).await.unwrap();
}