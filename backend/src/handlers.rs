use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use crate::types::*;
use crate::game;
use crate::utils::generate_room_name;
use crate::llm;

pub async fn player_ready_to_start(
    State(app_state): State<SharedAppState>,
    Json(request): Json<PlayerReadyToStartRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = match app_state.lock().unwrap().rooms.get(&request.room_name).cloned() {
        Some(arc) => arc,
        None => return Err(StatusCode::NOT_FOUND),
    };

    match game::handle_player_ready_to_start(game_data_arc, &request.player_id, &request.room_name).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

pub async fn submit_answer(
    State(app_state): State<SharedAppState>,
    Json(request): Json<SubmitAnswerRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&request.room_name).cloned()
    };
    
    match game_data_arc {
        Some(game_data) => {
            match game::submit_player_answer(&game_data, &request.player_id, request.answer) {
                Ok(all_submitted) => {
                    if all_submitted {
                        // Process answers asynchronously
                        let game_data_clone = game_data.clone();
                        tokio::spawn(async move {
                            game::process_all_answers(game_data_clone).await;
                        });
                    }
                    Ok(StatusCode::OK)
                }
                Err(_) => Err(StatusCode::BAD_REQUEST),
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn ready_for_next(
    State(app_state): State<SharedAppState>,
    Json(request): Json<ReadyForNextRequest>,
) -> Result<StatusCode, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&request.room_name).cloned()
    };
    
    match game_data_arc {
        Some(game_data) => {
            match game::handle_ready_for_next(&game_data, &request.player_id) {
                Ok(()) => Ok(StatusCode::OK),
                Err(_) => Err(StatusCode::NOT_FOUND),
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn generate_trivia() -> Result<String, StatusCode> {
    match llm::generate_trivia_questions().await {
        Ok(questions) => Ok(questions),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_room(
    State(app_state): State<SharedAppState>,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, StatusCode> {
    // Generate a unique room name
    let room_name = loop {
        let candidate = generate_room_name();
        let app = app_state.lock().unwrap();
        if !app.rooms.contains_key(&candidate) {
            break candidate;
        }
    };
    
    let (game_data_arc, response) = game::create_new_room(room_name.clone(), request.player_name);
    
    {
        let mut app = app_state.lock().unwrap();
        app.rooms.insert(room_name, game_data_arc);
    }
    
    Ok(Json(response))
}

pub async fn join_room(
    State(app_state): State<SharedAppState>,
    Json(request): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, StatusCode> {
    let game_data_arc = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&request.room_name).cloned()
    };

    match game_data_arc {
        Some(game_data) => {
            match game::join_existing_room(&game_data, request.player_name, request.room_name) {
                Ok(response) => Ok(Json(response)),
                Err(_) => Err(StatusCode::BAD_REQUEST),
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn get_players_in_room(
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