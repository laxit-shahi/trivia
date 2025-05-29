use axum::{
    extract::{ws::WebSocket, ws::WebSocketUpgrade, State},
    http::StatusCode,
    response::Response,
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    fs,
    path::Path,
};
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;
use rand;
use rand::seq::SliceRandom;
use chrono::{DateTime, Utc};
use serde_json;

mod llm;
mod types;
mod utils;
mod game;
mod handlers;
mod websocket;

use types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoggedQuestion {
    question: String,
    answer: String,
    category: String,
    timestamp: DateTime<Utc>,
    room_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct QuestionsLog {
    questions: Vec<LoggedQuestion>,
}

pub fn log_questions_to_file(questions: &[Question], category: &str, room_name: &str) {
    let questions_file = "questions.json";
    
    // Read existing questions or create new structure
    let mut questions_log = if Path::new(questions_file).exists() {
        match fs::read_to_string(questions_file) {
            Ok(content) => serde_json::from_str::<QuestionsLog>(&content).unwrap_or_else(|_| QuestionsLog {
                questions: Vec::new(),
            }),
            Err(_) => QuestionsLog {
                questions: Vec::new(),
            },
        }
    } else {
        QuestionsLog {
            questions: Vec::new(),
        }
    };
    
    // Add new questions
    let current_time = Utc::now();
    for question in questions {
        let logged_question = LoggedQuestion {
            question: question.question.clone(),
            answer: question.answer.clone(),
            category: category.to_string(),
            timestamp: current_time,
            room_name: room_name.to_string(),
        };
        questions_log.questions.push(logged_question);
    }
    
    // Write back to file
    match serde_json::to_string_pretty(&questions_log) {
        Ok(json_content) => {
            if let Err(e) = fs::write(questions_file, json_content) {
                tracing::error!("Failed to write questions log: {}", e);
            } else {
                tracing::info!("Logged {} questions for room {} in category {}", questions.len(), room_name, category);
            }
        }
        Err(e) => {
            tracing::error!("Failed to serialize questions log: {}", e);
        }
    }
}

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