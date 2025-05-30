use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, sync::Mutex};
use tokio::sync::broadcast;
use crate::llm::AnswerCorrectness;

// ============================================================================
// Core Game Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub score: u32,
    pub current_answer: Option<String>,
    pub ready_for_next: bool,
    pub is_ready_to_start: bool,
    pub is_host: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerResult {
    pub player_name: String,
    pub answer: String,
    pub correctness: AnswerCorrectness,
    pub correct_answer: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameState {
    WaitingForPlayers,
    InProgress { current_question: u32 },
    ShowingResults { current_question: u32 },
    Ended,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub players: HashMap<String, Player>,
    pub state: GameState,
    pub questions: Vec<Question>,
    pub questions_ready: bool,
    pub current_results: Vec<PlayerResult>,
    pub tx: broadcast::Sender<GameMessage>,
}

#[derive(Debug)]
pub struct AppState {
    pub rooms: HashMap<String, Arc<Mutex<GameData>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriviaSettings {
    pub instructions: String,
    pub category: String,
    pub difficulty: u32,
    pub num_of_questions: u32,
    pub age_group: String,
    pub hint_level: String,
}

// ============================================================================
// WebSocket Messages
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GameMessage {
    PlayerJoined { player: Player },
    PlayerReadyStateChanged { player_id: String, is_ready: bool },
    GameStarted { num_questions: u32 },
    QuestionPresented { question: String, question_number: u32 },
    AnswerSubmitted { player_id: String, answer: String },
    ResultsShown { results: Vec<PlayerResult>, correct_answer: String },
    ScoreAdjusted { results: Vec<PlayerResult> },
    NextQuestion,
    GameEnded { final_scores: Vec<Player> },
    ReturnedToLobby,
    QuestionsReady { num_questions: u32 },
    Error { message: String },
}

// ============================================================================
// API Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SubmitAnswerRequest {
    pub player_id: String,
    pub answer: String,
    pub room_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadyForNextRequest {
    pub player_id: String,
    pub room_name: String,
}

#[derive(Debug, Deserialize)]
pub struct PlayerReadyToStartRequest {
    pub player_id: String,
    pub room_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ReturnToLobbyRequest {
    pub player_id: String,
    pub room_name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    pub player_name: String,
}

#[derive(Debug, Deserialize)]
pub struct JoinRoomRequest {
    pub room_name: String,
    pub player_name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateRoomResponse {
    pub room_id: String,
    pub room_name: String,
    pub player: Player,
    pub is_host: bool,
}

#[derive(Debug, Serialize)]
pub struct JoinRoomResponse {
    pub room_id: String,
    pub room_name: String,
    pub player: Player,
    pub is_host: bool,
}

#[derive(Debug, Deserialize)]
pub struct AdjustScoreRequest {
    pub player_id: String, // host player ID
    pub room_name: String,
    pub target_player_name: String, // player whose score to adjust
    pub adjustment: i32, // +1 for upgrade, -1 for downgrade
}

// ============================================================================
// Type Aliases
// ============================================================================

pub type SharedAppState = Arc<Mutex<AppState>>; 