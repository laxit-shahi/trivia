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
};
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;
use rand;
use rand::seq::SliceRandom;
use tower_http::cors;

mod llm;
use llm::AnswerCorrectness;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Player {
    id: String,
    name: String,
    score: u32,
    current_answer: Option<String>,
    ready_for_next: bool,
    is_ready_to_start: bool,
    has_skipped_voting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Question {
    question: String,
    answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerResult {
    player_name: String,
    answer: String,
    correctness: AnswerCorrectness,
    correct_answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PartialAnswerVote {
    voter_id: String,
    target_player_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum GameMessage {
    PlayerJoined { player: Player },
    PlayerReadyStateChanged { player_id: String, is_ready: bool },
    GameStarted { num_questions: u32 },
    QuestionPresented { question: String, question_number: u32 },
    AnswerSubmitted { player_id: String, answer: String },
    ResultsShown { results: Vec<PlayerResult>, correct_answer: String },
    PartialVoteSubmitted { voter_id: String, target_player_id: String },
    VotingComplete { updated_results: Vec<PlayerResult> },
    NextQuestion,
    GameEnded { final_scores: Vec<Player> },
    Error { message: String },
    PlayerSkippedVoting { player_id: String },
}

#[derive(Debug, Clone, PartialEq)]
enum GameState {
    WaitingForPlayers,
    InProgress { current_question: u32 },
    ShowingResults { current_question: u32 },
    Ended,
}

#[derive(Debug, Clone)]
struct GameData {
    players: HashMap<String, Player>,
    state: GameState,
    questions: Vec<Question>,
    partial_votes: Vec<PartialAnswerVote>,
    current_results: Vec<PlayerResult>,
    tx: broadcast::Sender<GameMessage>,
}

type SharedAppState = Arc<Mutex<AppState>>;

#[derive(Debug, Deserialize)]
struct SubmitAnswerRequest {
    player_id: String,
    answer: String,
    room_name: String,
}

#[derive(Debug, Deserialize)]
struct ReadyForNextRequest {
    player_id: String,
    room_name: String,
}

#[derive(Debug, Deserialize)]
struct PlayerReadyToStartRequest {
    player_id: String,
    room_name: String,
}

#[derive(Debug, Deserialize)]
struct CreateRoomRequest {
    player_name: String,
}

#[derive(Debug, Deserialize)]
struct JoinRoomRequest {
    room_name: String,
    player_name: String,
}

#[derive(Debug, Deserialize)]
struct SkipVotingRequest {
    player_id: String,
    room_name: String,
}

#[derive(Debug, Serialize)]
struct CreateRoomResponse {
    room_id: String,
    room_name: String,
    player: Player,
    is_host: bool,
}

#[derive(Debug, Serialize)]
struct JoinRoomResponse {
    room_id: String,
    room_name: String,
    player: Player,
    is_host: bool,
}

#[derive(Debug)]
struct AppState {
    rooms: HashMap<String, Arc<Mutex<GameData>>>,
}

#[derive(Debug, Deserialize)]
struct PartialVoteRequest {
    voter_id: String,
    target_player_id: String,
    room_name: String,
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

async fn player_ready_to_start(
    State(app_state): State<SharedAppState>,
    Json(request): Json<PlayerReadyToStartRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = match app_state.lock().unwrap().rooms.get(&request.room_name).cloned() {
        Some(arc) => arc,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let player_id_clone = request.player_id.clone();
    let mut initiate_game_start_sequence = false;
    let room_name_clone = request.room_name.clone(); // Clone for async block

    // Scope 1: Update player ready state and decide if game should start
    {
        let mut game = game_data_arc.lock().unwrap();
        let current_game_state = game.state.clone();

        if current_game_state != GameState::WaitingForPlayers {
            info!("Player {} attempted to toggle ready in room {} which is not WaitingForPlayers (state: {:?})", 
                   player_id_clone, request.room_name, current_game_state);
            return Err(StatusCode::BAD_REQUEST);
        }
        
        let player_is_now_ready: bool; // Variable to store the player's new ready state
        {
            let player = game.players.get_mut(&request.player_id).ok_or_else(|| {
                info!("Player {} not found in room {} to toggle ready.", player_id_clone, request.room_name);
                StatusCode::NOT_FOUND
            })?;
            player.is_ready_to_start = !player.is_ready_to_start;
            player_is_now_ready = player.is_ready_to_start; // Capture the new state
        }

        if game.tx.send(GameMessage::PlayerReadyStateChanged {
            player_id: player_id_clone.clone(),
            is_ready: player_is_now_ready, // Use the captured state
        }).is_err() {
            info!("Failed to broadcast PlayerReadyStateChanged for player {}", player_id_clone);
        }

        let all_players_ready = game.players.values().all(|p| p.is_ready_to_start);
        let enough_players = game.players.len() >= 1;

        if all_players_ready && enough_players && current_game_state == GameState::WaitingForPlayers {
            initiate_game_start_sequence = true;
            // Do NOT change game.state or game.questions here yet. Do it after LLM call.
            info!("All players ready in room {}. Initiating LLM question generation.", request.room_name);
        }
    } // Game lock released

    if initiate_game_start_sequence {
        // Asynchronously generate questions
        let generated_questions_result = llm::generate_trivia_questions().await;

        let questions_to_use = match generated_questions_result {
            Ok(json_text) => {
                match parse_generated_questions(&json_text) {
                    Ok(parsed_q) if !parsed_q.is_empty() => {
                        info!("Successfully parsed {} questions from LLM for room {}", parsed_q.len(), room_name_clone);
                        parsed_q
                    }
                    Ok(_) => { // Parsed but empty
                        info!("LLM returned empty question set for room {}, falling back to default questions.", room_name_clone);
                        create_questions() 
                    }
                    Err(e) => {
                        info!("Failed to parse LLM questions for room {}: {}. Falling back to default questions.", room_name_clone, e);
                        create_questions()
                    }
                }
            }
            Err(e) => {
                info!("Failed to generate LLM questions for room {}: {}. Falling back to default questions.", room_name_clone, e);
                create_questions()
            }
        };

        // Scope 2: Update game state with questions and send start messages
        {
            let mut game = game_data_arc.lock().unwrap();
            // Double check state, another request might have started the game with default questions if LLM was very slow
            if game.state == GameState::WaitingForPlayers { 
                game.state = GameState::InProgress { current_question: 0 };
                game.questions = questions_to_use;
                let total_questions = game.questions.len() as u32;
                info!("Game starting in room {} with {} players, using {} questions.", 
                       room_name_clone, game.players.len(), total_questions);

                if game.tx.send(GameMessage::GameStarted { num_questions: total_questions }).is_err() {
                    info!("Failed to broadcast GameStarted for room {}", room_name_clone);
                }

                if let Some(question) = game.questions.get(0) {
                    if game.tx.send(GameMessage::QuestionPresented {
                        question: question.question.clone(),
                        question_number: 1,
                    }).is_err() {
                        info!("Failed to send first question for room {}", room_name_clone);
                    }
                } else {
                    info!("Error: No questions available (LLM or fallback) to start game in room {}. Reverting state.", room_name_clone);
                    game.state = GameState::WaitingForPlayers; 
                    let _ = game.tx.send(GameMessage::Error { message: "Failed to load questions for game start.".to_string() });
                    return Err(StatusCode::INTERNAL_SERVER_ERROR); // Indicate error if game couldn't start
                }
            } else {
                 info!("Game in room {} was already started. LLM questions were fetched but not used.", room_name_clone);
            }
        }
    }

    Ok(StatusCode::OK)
}

async fn submit_answer(
    State(app_state): State<SharedAppState>,
    Json(request): Json<SubmitAnswerRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&request.room_name).cloned()
    };
    
    match game_data_arc {
        Some(game_data) => {
            {
    let mut game = game_data.lock().unwrap();
    
                match game.state {
                    GameState::InProgress { current_question: _current_question } => {
    if let Some(player) = game.players.get_mut(&request.player_id) {
        player.current_answer = Some(request.answer.clone());
    } else {
        return Err(StatusCode::NOT_FOUND);
    }
    
                        // Check if all players have submitted
                        if !game.players.values().all(|p| p.current_answer.is_some()) {
                return Ok(StatusCode::OK);
                        }
                    }
                    _ => return Err(StatusCode::BAD_REQUEST),
                }
            }
            
            // Process answers with LLM
            let game_data_clone = game_data.clone();
            tokio::spawn(async move {
                let mut player_results = Vec::new();
                let current_question_text;
                let correct_answer;
                let current_question_index;
                
                {
                    let game = game_data_clone.lock().unwrap();
                    current_question_index = match game.state {
                        GameState::InProgress { current_question } => current_question,
                        _ => 0,
                    };
                    let current_question = &game.questions[current_question_index as usize];
                    current_question_text = current_question.question.clone();
                    correct_answer = current_question.answer.clone();
                }
                
                // Collect player data outside the lock
                let player_data: Vec<(String, String, String)> = {
                    let game = game_data_clone.lock().unwrap();
                    game.players.iter()
                        .filter_map(|(id, player)| {
                            player.current_answer.as_ref().map(|answer| {
                                (id.clone(), player.name.clone(), answer.clone())
                            })
                        })
                        .collect()
                };
                
                // Process answers with LLM (outside the lock)
                for (player_id, player_name, answer) in player_data {
                    let correctness = match llm::check_answer_correctness(
                        &current_question_text,
                        &correct_answer,
                        &answer,
                    ).await {
                        Ok(correctness) => correctness,
                        Err(_) => AnswerCorrectness::Wrong,
                    };
                    
                    player_results.push((player_id, correctness.clone(), PlayerResult {
                        player_name,
                        answer,
                        correctness: correctness.clone(),
                        correct_answer: correct_answer.clone(),
                    }));
                }
                
                // Update game state with results
                {
                    let mut game = game_data_clone.lock().unwrap();
                    
                    // Update player scores based on initial LLM check
                    for (player_id, correctness, _) in &player_results {
                        if let Some(player) = game.players.get_mut(player_id) {
                            let points = match correctness {
                                AnswerCorrectness::Correct => 2, // 2 points for correct
                                AnswerCorrectness::Partial => 0, // 0 points initially for partial
                                AnswerCorrectness::Wrong => 0,   // 0 points for wrong
                            };
                            player.score += points;
                            player.current_answer = None; // Reset for next round
                            player.ready_for_next = false; // Reset for next round
                        }
                    }
                    
                    let final_results_for_display: Vec<PlayerResult> = player_results.into_iter()
                        .map(|(_, _, result)| result)
                        .collect();
                    
                    game.current_results = final_results_for_display.clone();
                    game.state = GameState::ShowingResults { current_question: current_question_index };
                    
                    let _ = game.tx.send(GameMessage::ResultsShown {
                        results: final_results_for_display,
                        correct_answer: correct_answer.clone(),
                    });
                }
            });
            
            Ok(StatusCode::OK)
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn ready_for_next(
    State(app_state): State<SharedAppState>,
    Json(request): Json<ReadyForNextRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&request.room_name).cloned()
    };
    
    match game_data_arc {
        Some(game_data) => {
    let mut game = game_data.lock().unwrap();
    
    if let Some(player) = game.players.get_mut(&request.player_id) {
        player.ready_for_next = true;
        
        let all_ready = game.players.values().all(|p| p.ready_for_next);
        
        if all_ready {
            if let GameState::ShowingResults { current_question } = game.state {
                let next_question = current_question + 1;
                let max_questions = game.questions.len() as u32;

                if next_question >= max_questions {
                    game.state = GameState::Ended;
                    let final_scores: Vec<Player> = game.players.values().cloned().collect();
                    let _ = game.tx.send(GameMessage::GameEnded { final_scores });
                } else {
                    game.state = GameState::InProgress { current_question: next_question };
                    if let Some(question) = game.questions.get(next_question as usize) {
                        let _ = game.tx.send(GameMessage::QuestionPresented {
                            question: question.question.clone(),
                            question_number: next_question + 1,
                        });
                        // Reset ready_for_next and has_skipped_voting for all players
                        for p in game.players.values_mut() {
                            p.ready_for_next = false;
                            p.has_skipped_voting = false; // Reset skip status
                        }
                    }
                }
            }
        }
        
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<SharedAppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Response {
    let room_name = params.get("room").cloned().unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, app_state, room_name))
}

async fn handle_socket(socket: WebSocket, app_state: SharedAppState, room_name: String) {
    let game_data_opt = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&room_name).cloned()
    };
    
    if let Some(game_data) = game_data_opt {
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
}

async fn generate_trivia(
) -> Result<String, StatusCode> {
    match llm::generate_trivia_questions().await {
        Ok(questions) => Ok(questions),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn check_and_process_voting_completion(game: &mut GameData) {
    // Determine players who have partial answers that need voting on
    let partial_answer_player_ids: Vec<String> = game.current_results.iter()
        .filter_map(|result| {
            if matches!(result.correctness, AnswerCorrectness::Partial) {
                game.players.iter()
                    .find(|(_, player)| player.name == result.player_name)
                    .map(|(id, _)| id.clone())
            } else {
                None
            }
        })
        .collect();

    if partial_answer_player_ids.is_empty() {
        let updated_results = game.current_results.clone();
        let _ = game.tx.send(GameMessage::VotingComplete { updated_results });
        return;
    }

    let mut all_voting_actions_complete = true;
    for player_in_game in game.players.values() {
        if player_in_game.has_skipped_voting {
            continue; 
        }

        for target_partial_player_id in &partial_answer_player_ids {
            if &player_in_game.id == target_partial_player_id {
                continue; 
            }
            let has_voted_on_this_target = game.partial_votes.iter()
                .any(|vote| vote.voter_id == player_in_game.id && &vote.target_player_id == target_partial_player_id);
            
            if !has_voted_on_this_target {
                all_voting_actions_complete = false;
                break; 
            }
        }
        if !all_voting_actions_complete {
            break; 
        }
    }

    if all_voting_actions_complete {
        let mut vote_counts: HashMap<String, usize> = HashMap::new();
        for vote in &game.partial_votes {
            if let Some(voter) = game.players.get(&vote.voter_id) {
                if !voter.has_skipped_voting {
                    *vote_counts.entry(vote.target_player_id.clone()).or_insert(0) += 1;
                }
            }
        }
        
        let active_voters_count = game.players.values().filter(|p| !p.has_skipped_voting).count();
        let majority_threshold = if active_voters_count > 0 { active_voters_count / 2 } else { 0 };

        for (player_id, vote_count) in vote_counts {
            if let Some(player_to_score) = game.players.get_mut(&player_id) {
                let is_target_partial = partial_answer_player_ids.contains(&player_id);
                if is_target_partial && vote_count > majority_threshold {
                    player_to_score.score += 2; 
                }
            }
        }
        
        let updated_results = game.current_results.clone(); 
        let _ = game.tx.send(GameMessage::VotingComplete { updated_results });
    }
}

async fn vote_partial_answer(
    State(app_state): State<SharedAppState>,
    Json(request): Json<PartialVoteRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&request.room_name).cloned()
    };
    
    match game_data_arc {
        Some(game_data) => {
            let mut game = game_data.lock().unwrap();
            
            match game.state {
                GameState::ShowingResults { .. } => {
                    if let Some(voter) = game.players.get(&request.voter_id) {
                        if voter.has_skipped_voting {
                            return Err(StatusCode::FORBIDDEN); 
                        }
                    } else {
                        return Err(StatusCode::NOT_FOUND); 
                    }

                    if request.voter_id == request.target_player_id {
                        return Err(StatusCode::BAD_REQUEST); 
                    }
                    
                    let target_player_name = game.players.get(&request.target_player_id).map(|p| p.name.clone());
                    let has_partial_answer = target_player_name.map_or(false, |name| {
                        game.current_results.iter().any(|result| 
                            result.player_name == name && 
                            matches!(result.correctness, AnswerCorrectness::Partial)
                        )
                    });
                    
                    if !has_partial_answer {
                        return Err(StatusCode::BAD_REQUEST); 
                    }
                    
                    game.partial_votes.retain(|vote| 
                        !(vote.voter_id == request.voter_id && vote.target_player_id == request.target_player_id)
                    );
                    
                    game.partial_votes.push(PartialAnswerVote {
                        voter_id: request.voter_id.clone(),
                        target_player_id: request.target_player_id.clone(),
                    });
                    
                    let _ = game.tx.send(GameMessage::PartialVoteSubmitted {
                        voter_id: request.voter_id,
                        target_player_id: request.target_player_id,
                    });
                    
                    check_and_process_voting_completion(&mut game);
                    
                    Ok(StatusCode::OK)
                }
                _ => Err(StatusCode::BAD_REQUEST),
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn skip_voting(
    State(app_state): State<SharedAppState>,
    Json(request): Json<SkipVotingRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = match app_state.lock().unwrap().rooms.get(&request.room_name).cloned() {
        Some(arc) => arc,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let mut game = game_data_arc.lock().unwrap();

    match game.state {
        GameState::ShowingResults { .. } => {
            if let Some(player) = game.players.get_mut(&request.player_id) {
                player.has_skipped_voting = true;

                let _ = game.tx.send(GameMessage::PlayerSkippedVoting { player_id: request.player_id.clone() });

                check_and_process_voting_completion(&mut game);
                Ok(StatusCode::OK)
            } else {
                Err(StatusCode::NOT_FOUND) 
            }
        }
        _ => Err(StatusCode::BAD_REQUEST), 
    }
}

async fn create_room(
    State(app_state): State<SharedAppState>,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, StatusCode> {
    let room_id = Uuid::new_v4().to_string();
    let (tx, _rx) = broadcast::channel(100);
    
    // Generate a unique room name
    let room_name = loop {
        let candidate = generate_room_name();
        let app = app_state.lock().unwrap();
        if !app.rooms.contains_key(&candidate) {
            break candidate;
        }
        // If there's a collision, try again (very unlikely with our format)
    };
    
    let player = Player {
        id: Uuid::new_v4().to_string(),
        name: request.player_name.clone(),
        score: 0,
        current_answer: None,
        ready_for_next: false,
        is_ready_to_start: false,
        has_skipped_voting: false,
    };
    
    let mut players = HashMap::new();
    players.insert(player.id.clone(), player.clone());
    
    let game_data = GameData {
        players,
        state: GameState::WaitingForPlayers,
        questions: Vec::new(),
        partial_votes: Vec::new(),
        current_results: Vec::new(),
        tx,
    };
    
    let game_data_arc = Arc::new(Mutex::new(game_data));
    
    {
        let mut app = app_state.lock().unwrap();
        app.rooms.insert(room_name.clone(), game_data_arc.clone());
    }
    
    // Send player joined message
    {
        let game = game_data_arc.lock().unwrap();
        let _ = game.tx.send(GameMessage::PlayerJoined { player: player.clone() });
    }
    
    Ok(Json(CreateRoomResponse {
        room_id,
        room_name: room_name.clone(),
        player,
        is_host: true,
    }))
}

async fn join_room(
    State(app_state): State<SharedAppState>,
    Json(request): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&request.room_name).cloned()
    };

    match game_data_arc {
        Some(game_data) => {
            let mut game = game_data.lock().unwrap();

            match game.state {
                GameState::WaitingForPlayers => {
                    let mut existing_player_id_to_remove: Option<String> = None;
                    for p in game.players.values() {
                        if p.name == request.player_name {
                            existing_player_id_to_remove = Some(p.id.clone());
                            break;
                        }
                    }

                    if let Some(id_to_remove) = existing_player_id_to_remove {
                        game.players.remove(&id_to_remove);
                        info!("Player {} rejoining room {}, replacing old entry.", request.player_name, request.room_name);
                    }

                    let is_host = game.players.is_empty();

                    let new_player_id = Uuid::new_v4().to_string();
                    let player = Player {
                        id: new_player_id.clone(),
                        name: request.player_name.clone(),
                        score: 0, 
                        current_answer: None,
                        ready_for_next: false,
                        is_ready_to_start: false, 
                        has_skipped_voting: false,
                    };

                    game.players.insert(player.id.clone(), player.clone());
                    
                    let _ = game.tx.send(GameMessage::PlayerJoined { player: player.clone() });

                    Ok(Json(JoinRoomResponse {
                        room_id: request.room_name.clone(), 
                        room_name: request.room_name,
                        player,
                        is_host,
                    }))
                }
                _ => {
                    info!("Player {} attempted to join room {} which is not in WaitingForPlayers state.", request.player_name, request.room_name);
                    Err(StatusCode::BAD_REQUEST) 
                }
            }
        }
        None => {
            info!("Player {} attempted to join non-existent room {}.", request.player_name, request.room_name);
            Err(StatusCode::NOT_FOUND) 
        }
    }
}

fn generate_room_name() -> String {
    let adjectives = vec![
        "Quick", "Smart", "Bright", "Sharp", "Clever", "Swift", "Bold", "Wise", 
        "Keen", "Fast", "Super", "Grand", "Epic", "Cool", "Wild", "Fun",
        "Happy", "Lucky", "Magic", "Power", "Turbo", "Ultra", "Mega", "Star"
    ];
    
    let nouns = vec![
        "Quiz", "Brain", "Mind", "Think", "Know", "Learn", "Study", "Fact",
        "Smart", "Genius", "Expert", "Master", "Champion", "Winner", "Hero", "Star",
        "Tiger", "Eagle", "Lion", "Wolf", "Bear", "Shark", "Dragon", "Phoenix"
    ];
    
    let mut rng = rand::thread_rng();
    let adjective = adjectives.choose(&mut rng).unwrap_or(&"Cool"); // Added unwrap_or for safety
    let noun = nouns.choose(&mut rng).unwrap_or(&"Quiz"); // Added unwrap_or for safety
    let number = rand::random::<u16>() % 1000;
    
    format!("{}-{}-{}", adjective, noun, number)
}

async fn get_players_in_room(
    State(app_state): State<SharedAppState>,
    axum::extract::Path(room_name): axum::extract::Path<String>,
) -> Result<Json<Vec<Player>>, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&room_name).cloned()
    };

    match game_data_arc {
        Some(game_data) => {
            let game = game_data.lock().unwrap();
            let players: Vec<Player> = game.players.values().cloned().collect();
            Ok(Json(players))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    let app_state = Arc::new(Mutex::new(AppState {
        rooms: HashMap::new(),
    }));
    
    let app = axum::Router::new()
        .route("/api/create-room", axum::routing::post(create_room))
        .route("/api/join-room", axum::routing::post(join_room))
        .route("/api/player-ready-to-start", axum::routing::post(player_ready_to_start))
        .route("/api/submit-answer", axum::routing::post(submit_answer))
        .route("/api/ready-next", axum::routing::post(ready_for_next))
        .route("/api/generate-trivia", axum::routing::get(generate_trivia))
        .route("/api/vote-partial-answer", axum::routing::post(vote_partial_answer))
        .route("/api/skip-voting", axum::routing::post(skip_voting))
        .route("/ws", axum::routing::get(websocket_handler))
        .route("/api/room/:room_name/players", axum::routing::get(get_players_in_room))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(app_state);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    info!("Server running on http://0.0.0.0:3001");
    
    axum::serve(listener, app).await.unwrap();
}