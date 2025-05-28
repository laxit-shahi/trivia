use axum::{
    extract::{ws::WebSocket, ws::WebSocketUpgrade, State},
    http::StatusCode,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing::info;
use uuid::Uuid;

mod llm;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Player {
    id: String,
    name: String,
    score: u32,
    current_answer: Option<String>,
    ready_for_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Question {
    question: String,
    answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum GameMessage {
    PlayerJoined { player: Player },
    GameStarted,
    QuestionPresented { question: String, question_number: u32 },
    AnswerSubmitted { player_id: String, answer: String },
    ResultsShown { results: Vec<PlayerResult> },
    NextQuestion,
    GameEnded { final_scores: Vec<Player> },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerResult {
    player_name: String,
    answer: String,
    correct: bool,
}

#[derive(Debug, Clone)]
enum GameState {
    WaitingForPlayers,
    InProgress { current_question: u32 },
    ShowingResults { current_question: u32 },
    Ended,
}

#[derive(Debug)]
struct GameData {
    players: HashMap<String, Player>,
    state: GameState,
    questions: Vec<Question>,
    tx: broadcast::Sender<GameMessage>,
}

type SharedGameData = Arc<Mutex<GameData>>;

#[derive(Debug, Deserialize)]
struct JoinGameRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct SubmitAnswerRequest {
    player_id: String,
    answer: String,
}

#[derive(Debug, Deserialize)]
struct ReadyForNextRequest {
    player_id: String,
}

#[derive(Debug, Serialize)]
struct JoinGameResponse {
    player: Player,
    is_host: bool,
}

fn create_questions() -> Vec<Question> {
    vec![
        Question {
            question: "What is the capital of France?".to_string(),
            answer: "Paris".to_string(),
        },
        Question {
            question: "What is 2 + 2?".to_string(),
            answer: "4".to_string(),
        },
        Question {
            question: "What is the largest planet in our solar system?".to_string(),
            answer: "Jupiter".to_string(),
        },
        Question {
            question: "Who painted the Mona Lisa?".to_string(),
            answer: "Leonardo da Vinci".to_string(),
        },
        Question {
            question: "What is the chemical symbol for gold?".to_string(),
            answer: "Au".to_string(),
        },
        Question {
            question: "In which year did World War II end?".to_string(),
            answer: "1945".to_string(),
        },
        Question {
            question: "What is the smallest country in the world?".to_string(),
            answer: "Vatican City".to_string(),
        },
        Question {
            question: "How many continents are there?".to_string(),
            answer: "7".to_string(),
        },
        Question {
            question: "What is the longest river in the world?".to_string(),
            answer: "Nile".to_string(),
        },
        Question {
            question: "What gas do plants absorb from the atmosphere?".to_string(),
            answer: "Carbon dioxide".to_string(),
        },
    ]
}

fn parse_generated_questions(json_text: &str) -> Result<Vec<Question>, Box<dyn std::error::Error>> {
    let parsed: serde_json::Value = serde_json::from_str(json_text)?;
    
    let questions_array = parsed.get("questions")
        .and_then(|q| q.as_array())
        .ok_or("No 'questions' array found in response")?;
    
    let mut questions = Vec::new();
    
    for question_obj in questions_array {
        let question_text = question_obj.get("question")
            .and_then(|q| q.as_str())
            .ok_or("Missing 'question' field")?;
        
        let answer_text = question_obj.get("answer")
            .and_then(|a| a.as_str())
            .ok_or("Missing 'answer' field")?;
        
        questions.push(Question {
            question: question_text.to_string(),
            answer: answer_text.to_string(),
        });
    }
    
    Ok(questions)
}

async fn join_game(
    State(game_data): State<SharedGameData>,
    Json(request): Json<JoinGameRequest>,
) -> Result<Json<JoinGameResponse>, StatusCode> {
    let mut game = game_data.lock().unwrap();
    
    match game.state {
        GameState::WaitingForPlayers => {
            let is_host = game.players.is_empty(); // First player is the host
            
            let player = Player {
                id: Uuid::new_v4().to_string(),
                name: request.name,
                score: 0,
                current_answer: None,
                ready_for_next: false,
            };
            
            game.players.insert(player.id.clone(), player.clone());
            
            let _ = game.tx.send(GameMessage::PlayerJoined { player: player.clone() });
            
            Ok(Json(JoinGameResponse {
                player,
                is_host,
            }))
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

async fn start_game(State(game_data): State<SharedGameData>) -> Result<StatusCode, StatusCode> {
    // First, generate new trivia questions
    let generated_questions = match llm::generate_trivia_questions().await {
        Ok(json_text) => {
            match parse_generated_questions(&json_text) {
                Ok(questions) => questions,
                Err(_) => {
                    // Fallback to static questions if parsing fails
                    create_questions()
                }
            }
        }
        Err(_) => {
            // Fallback to static questions if LLM call fails
            create_questions()
        }
    };
    
    let mut game = game_data.lock().unwrap();
    
    match game.state {
        GameState::WaitingForPlayers => {
            if game.players.is_empty() {
                return Err(StatusCode::BAD_REQUEST);
            }
            
            // Update the game with the generated questions
            game.questions = generated_questions;
            game.state = GameState::InProgress { current_question: 0 };
            
            let _ = game.tx.send(GameMessage::GameStarted);
            
            // Send first question
            if let Some(question) = game.questions.get(0) {
                let _ = game.tx.send(GameMessage::QuestionPresented {
                    question: question.question.clone(),
                    question_number: 1,
                });
            }
            
            Ok(StatusCode::OK)
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

async fn submit_answer(
    State(game_data): State<SharedGameData>,
    Json(request): Json<SubmitAnswerRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut game = game_data.lock().unwrap();
    
    // First, update the player's answer
    if let Some(player) = game.players.get_mut(&request.player_id) {
        player.current_answer = Some(request.answer.clone());
    } else {
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Send the answer submitted message
    let _ = game.tx.send(GameMessage::AnswerSubmitted {
        player_id: request.player_id.clone(),
        answer: request.answer,
    });
    
    // Check if all players have submitted answers
    let all_answered = game.players.values().all(|p| p.current_answer.is_some());
    
    if all_answered {
        if let GameState::InProgress { current_question } = game.state {
            // Get the correct answer first
            let correct_answer_text = if let Some(correct_answer) = game.questions.get(current_question as usize) {
                correct_answer.answer.clone()
            } else {
                return Ok(StatusCode::OK);
            };
            
            let mut results = Vec::new();
            
            // Process all players' answers
            for player in game.players.values_mut() {
                let correct = player.current_answer.as_ref()
                    .map(|ans| ans.trim().eq_ignore_ascii_case(correct_answer_text.trim()))
                    .unwrap_or(false);
                
                if correct {
                    player.score += 1;
                }
                
                results.push(PlayerResult {
                    player_name: player.name.clone(),
                    answer: player.current_answer.clone().unwrap_or_default(),
                    correct,
                });
                
                player.current_answer = None;
                player.ready_for_next = false;
            }
            
            game.state = GameState::ShowingResults { current_question };
            
            let _ = game.tx.send(GameMessage::ResultsShown { results });
        }
    }
    
    Ok(StatusCode::OK)
}

async fn ready_for_next(
    State(game_data): State<SharedGameData>,
    Json(request): Json<ReadyForNextRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut game = game_data.lock().unwrap();
    
    if let Some(player) = game.players.get_mut(&request.player_id) {
        player.ready_for_next = true;
        
        // Check if all players are ready
        let all_ready = game.players.values().all(|p| p.ready_for_next);
        
        if all_ready {
            if let GameState::ShowingResults { current_question } = game.state {
                let next_question = current_question + 1;
                
                if next_question >= 10 {
                    // Game ended
                    game.state = GameState::Ended;
                    let final_scores: Vec<Player> = game.players.values().cloned().collect();
                    let _ = game.tx.send(GameMessage::GameEnded { final_scores });
                } else {
                    // Next question
                    game.state = GameState::InProgress { current_question: next_question };
                    
                    if let Some(question) = game.questions.get(next_question as usize) {
                        let _ = game.tx.send(GameMessage::QuestionPresented {
                            question: question.question.clone(),
                            question_number: next_question + 1,
                        });
                    }
                }
            }
        }
        
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(game_data): State<SharedGameData>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, game_data))
}

async fn handle_socket(socket: WebSocket, game_data: SharedGameData) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = {
        let game = game_data.lock().unwrap();
        game.tx.subscribe()
    };
    
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&msg).unwrap(),
                ))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_)) = receiver.next().await {
            // We don't need to handle incoming WebSocket messages for this app
            // All communication goes through HTTP endpoints
        }
    });
    
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
        },
    }
}

async fn generate_trivia(
) -> Result<String, StatusCode> {
    match llm::generate_trivia_questions().await {
        Ok(questions) => Ok(questions),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    let (tx, _rx) = broadcast::channel(100);
    
    let game_data = Arc::new(Mutex::new(GameData {
        players: HashMap::new(),
        state: GameState::WaitingForPlayers,
        questions: create_questions(),
        tx,
    }));
    
    let app = Router::new()
        .route("/api/join", post(join_game))
        .route("/api/start", post(start_game))
        .route("/api/submit-answer", post(submit_answer))
        .route("/api/ready-next", post(ready_for_next))
        .route("/api/generate-trivia", get(generate_trivia))
        .route("/ws", get(websocket_handler))
        .layer(CorsLayer::permissive())
        .with_state(game_data);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    info!("Server running on http://0.0.0.0:3001");
    
    axum::serve(listener, app).await.unwrap();
}
