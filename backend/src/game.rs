use crate::utils::{create_questions, parse_generated_questions};
use crate::llm;
use crate::llm::AnswerCorrectness;
use crate::log_questions_to_file;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;
use crate::{Player, GameState, GameData, GameMessage, PlayerResult, CreateRoomResponse, JoinRoomResponse};

async fn generate_questions_async(game_data_arc: Arc<Mutex<GameData>>, room_name: String) {
    info!("Starting background question generation for room {}", room_name);
    
    let questions_to_use = match llm::generate_trivia_questions().await {
        Ok(json_text) => {
            match parse_generated_questions(&json_text) {
                Ok(parsed_q) if !parsed_q.is_empty() => {
                    info!("Successfully generated {} questions for room {}", parsed_q.len(), room_name);
                    parsed_q
                }
                Ok(_) => {
                    info!("LLM returned empty question set for room {}, using default questions. Response was: {}", room_name, json_text);
                    create_questions() 
                }
                Err(e) => {
                    info!("Failed to parse LLM questions for room {}: {}. Using default questions. LLM response was: {}", room_name, e, json_text);
                    create_questions()
                }
            }
        }
        Err(e) => {
            info!("Failed to generate LLM questions for room {}: {}. Using default questions.", room_name, e);
            create_questions()
        }
    };
    
    // Update the game data with new questions
    let should_start_game = {
        let mut game = game_data_arc.lock().unwrap();
        game.questions = questions_to_use.clone();
        game.questions_ready = true;
        
        let num_questions = questions_to_use.len() as u32;
        let _ = game.tx.send(GameMessage::QuestionsReady { num_questions });
        
        // Check if we should auto-start the game (all players ready)
        let all_players_ready = game.players.values().all(|p| p.is_ready_to_start);
        let enough_players = game.players.len() >= 1;
        let is_waiting = matches!(game.state, GameState::WaitingForPlayers);
        
        all_players_ready && enough_players && is_waiting
    };
    
    // If all players were ready, start the game now
    if should_start_game {
        info!("Questions ready and all players ready in room {}. Auto-starting game.", room_name);
        if let Err(e) = start_game_sequence(game_data_arc, &room_name).await {
            info!("Failed to auto-start game in room {}: {}", room_name, e);
        }
    }
}

pub async fn handle_player_ready_to_start(
    game_data_arc: Arc<Mutex<GameData>>, 
    player_id: &str, 
    room_name: &str
) -> Result<(), String> {
    let player_id_clone = player_id.to_string();
    let room_name_clone = room_name.to_string();
    let mut initiate_game_start_sequence = false;

    // Update player ready state and decide if game should start
    {
        let mut game = game_data_arc.lock().unwrap();
        let current_game_state = game.state.clone();

        if current_game_state != GameState::WaitingForPlayers {
            info!("Player {} attempted to toggle ready in room {} which is not WaitingForPlayers (state: {:?})", 
                   player_id_clone, room_name, current_game_state);
            return Err("Game not in waiting state".to_string());
        }
        
        let player_is_now_ready: bool;
        {
            let player = game.players.get_mut(player_id).ok_or_else(|| {
                info!("Player {} not found in room {} to toggle ready.", player_id_clone, room_name);
                "Player not found".to_string()
            })?;
            player.is_ready_to_start = !player.is_ready_to_start;
            player_is_now_ready = player.is_ready_to_start;
        }

        if game.tx.send(GameMessage::PlayerReadyStateChanged {
            player_id: player_id_clone.clone(),
            is_ready: player_is_now_ready,
        }).is_err() {
            info!("Failed to broadcast PlayerReadyStateChanged for player {}", player_id_clone);
        }

        let all_players_ready = game.players.values().all(|p| p.is_ready_to_start);
        let enough_players = game.players.len() >= 1;
        let questions_ready = game.questions_ready;

        if all_players_ready && enough_players && questions_ready && current_game_state == GameState::WaitingForPlayers {
            initiate_game_start_sequence = true;
            info!("All players ready in room {}. Starting game with pre-generated questions.", room_name);
        } else if all_players_ready && enough_players && !questions_ready && current_game_state == GameState::WaitingForPlayers {
            info!("All players ready in room {} but questions not ready yet. Waiting for question generation to complete.", room_name);
        }
    }

    if initiate_game_start_sequence {
        start_game_sequence(game_data_arc, &room_name_clone).await?;
    }

    Ok(())
}

async fn start_game_sequence(game_data_arc: Arc<Mutex<GameData>>, room_name: &str) -> Result<(), String> {
    // Questions are already generated when room was created, so just start the game
    {
        let mut game = game_data_arc.lock().unwrap();
        if game.state == GameState::WaitingForPlayers { 
            // Questions should already be available
            if game.questions.is_empty() {
                info!("Error: No questions available in room {}. This shouldn't happen.", room_name);
                return Err("No questions available".to_string());
            }
            
            game.state = GameState::InProgress { current_question: 0 };
            
            // Log questions to file
            log_questions_to_file(&game.questions, "General", room_name);
            
            let total_questions = game.questions.len() as u32;
            info!("Game starting in room {} with {} players, using {} pre-generated questions.", 
                   room_name, game.players.len(), total_questions);

            if game.tx.send(GameMessage::GameStarted { num_questions: total_questions }).is_err() {
                info!("Failed to broadcast GameStarted for room {}", room_name);
            }

            if let Some(question) = game.questions.get(0) {
                if game.tx.send(GameMessage::QuestionPresented {
                    question: question.question.clone(),
                    question_number: 1,
                }).is_err() {
                    info!("Failed to send first question for room {}", room_name);
                }
            }
        } else {
             info!("Game in room {} was already started.", room_name);
        }
    }

    Ok(())
}

pub fn submit_player_answer(
    game_data: &Arc<Mutex<GameData>>, 
    player_id: &str, 
    answer: String
) -> Result<bool, String> {
    let mut game = game_data.lock().unwrap();
    
    match game.state {
        GameState::InProgress { current_question: _current_question } => {
            if let Some(player) = game.players.get_mut(player_id) {
                player.current_answer = Some(answer);
            } else {
                return Err("Player not found".to_string());
            }
            
            // Check if all players have submitted
            Ok(game.players.values().all(|p| p.current_answer.is_some()))
        }
        _ => Err("Game not in progress".to_string()),
    }
}

pub async fn process_all_answers(game_data_arc: Arc<Mutex<GameData>>) {
    let mut player_results = Vec::new();
    let current_question_text;
    let correct_answer;
    let current_question_index;
    
    {
        let game = game_data_arc.lock().unwrap();
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
        let game = game_data_arc.lock().unwrap();
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
            Err(_) => llm::AnswerCorrectness::Wrong,
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
        let mut game = game_data_arc.lock().unwrap();
        
        // Update player scores based on LLM check
        for (player_id, correctness, _) in &player_results {
            if let Some(player) = game.players.get_mut(player_id) {
                let points = match correctness {
                    llm::AnswerCorrectness::Correct => 2,
                    llm::AnswerCorrectness::Partial => 1,
                    llm::AnswerCorrectness::Wrong => 0,
                };
                player.score += points;
                player.current_answer = None;
                player.ready_for_next = false;
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
}

pub fn handle_ready_for_next(
    game_data: &Arc<Mutex<GameData>>, 
    player_id: &str
) -> Result<(), String> {
    let mut game = game_data.lock().unwrap();
    
    if let Some(player) = game.players.get_mut(player_id) {
        // Only allow the host to advance the game
        if !player.is_host {
            return Err("Only the host can advance to the next question".to_string());
        }
        
        player.ready_for_next = true;
        
        // Host is ready, so advance the game immediately
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
                    // Reset ready_for_next for all players
                    for p in game.players.values_mut() {
                        p.ready_for_next = false;
                    }
                }
            }
        }
        
        Ok(())
    } else {
        Err("Player not found".to_string())
    }
}

pub fn handle_score_adjustment(
    game_data: &Arc<Mutex<GameData>>, 
    host_player_id: &str,
    target_player_name: &str,
    adjustment: i32
) -> Result<(), String> {
    let mut game = game_data.lock().unwrap();
    
    // Verify the requesting player is the host
    if let Some(host_player) = game.players.get(host_player_id) {
        if !host_player.is_host {
            return Err("Only the host can adjust scores".to_string());
        }
    } else {
        return Err("Host player not found".to_string());
    }
    
    // Find the result to adjust
    if let Some(result) = game.current_results.iter_mut().find(|r| r.player_name == target_player_name) {
        // Calculate new correctness based on adjustment
        let new_correctness = match (&result.correctness, adjustment) {
            (AnswerCorrectness::Wrong, 1) => AnswerCorrectness::Partial,
            (AnswerCorrectness::Partial, 1) => AnswerCorrectness::Correct,
            (AnswerCorrectness::Correct, 1) => AnswerCorrectness::Correct, // Stay at max
            (AnswerCorrectness::Correct, -1) => AnswerCorrectness::Partial,
            (AnswerCorrectness::Partial, -1) => AnswerCorrectness::Wrong,
            (AnswerCorrectness::Wrong, -1) => AnswerCorrectness::Wrong, // Stay at min
            _ => return Err("Invalid adjustment value".to_string()),
        };
        
        // Calculate score change
        let old_score = match result.correctness {
            AnswerCorrectness::Correct => 2,
            AnswerCorrectness::Partial => 1,
            AnswerCorrectness::Wrong => 0,
        };
        
        let new_score = match new_correctness {
            AnswerCorrectness::Correct => 2,
            AnswerCorrectness::Partial => 1,
            AnswerCorrectness::Wrong => 0,
        };
        
        let score_diff = new_score as i32 - old_score as i32;
        
        // Update the result
        result.correctness = new_correctness;
        
        // Update the player's score
        if let Some(player) = game.players.values_mut().find(|p| p.name == target_player_name) {
            if score_diff >= 0 {
                player.score += score_diff as u32;
            } else {
                let abs_diff = (-score_diff) as u32;
                if player.score >= abs_diff {
                    player.score -= abs_diff;
                } else {
                    player.score = 0;
                }
            }
        }
        
        // Broadcast the updated results
        let _ = game.tx.send(GameMessage::ScoreAdjusted {
            results: game.current_results.clone(),
        });
        
        Ok(())
    } else {
        Err("Player result not found".to_string())
    }
}

pub async fn create_new_room(room_name: String, player_name: String) -> (Arc<Mutex<GameData>>, CreateRoomResponse) {
    let room_id = Uuid::new_v4().to_string();
    let (tx, _rx) = broadcast::channel(100);
    
    let player = Player {
        id: Uuid::new_v4().to_string(),
        name: player_name.clone(),
        score: 0,
        current_answer: None,
        ready_for_next: false,
        is_ready_to_start: false,
        is_host: true,
    };
    
    let mut players = HashMap::new();
    players.insert(player.id.clone(), player.clone());
    
    // Start with default questions immediately, generate new ones in background
    let game_data = GameData {
        players,
        state: GameState::WaitingForPlayers,
        questions: create_questions(), // Start with default questions
        questions_ready: false, // Mark as not ready until AI questions are generated
        current_results: Vec::new(),
        tx,
    };
    
    let game_data_arc = Arc::new(Mutex::new(game_data));
    
    // Send player joined message
    {
        let game = game_data_arc.lock().unwrap();
        let _ = game.tx.send(GameMessage::PlayerJoined { player: player.clone() });
    }
    
    // Start generating questions asynchronously
    let game_data_clone = game_data_arc.clone();
    let room_name_clone = room_name.clone();
    tokio::spawn(async move {
        generate_questions_async(game_data_clone, room_name_clone).await;
    });
    
    let response = CreateRoomResponse {
        room_id,
        room_name: room_name.clone(),
        player,
        is_host: true,
    };
    
    (game_data_arc, response)
}

pub async fn join_existing_room(
    game_data: &Arc<Mutex<GameData>>, 
    player_name: String, 
    room_name: String
) -> Result<JoinRoomResponse, String> {
    let game_state = {
        let game = game_data.lock().unwrap();
        game.state.clone()
    };

    match game_state {
        GameState::WaitingForPlayers | GameState::Ended => {
            // If game has ended, reset it and generate new questions
            if game_state == GameState::Ended {
                // Generate new questions for the reset game
                info!("Regenerating trivia questions for reset room {}", room_name);
                let questions_to_use = match llm::generate_trivia_questions().await {
                    Ok(json_text) => {
                        match parse_generated_questions(&json_text) {
                            Ok(parsed_q) if !parsed_q.is_empty() => {
                                info!("Successfully regenerated {} questions for room {}", parsed_q.len(), room_name);
                                parsed_q
                            }
                            Ok(_) => {
                                info!("LLM returned empty question set for room {}, using default questions. Response was: {}", room_name, json_text);
                                create_questions() 
                            }
                            Err(e) => {
                                info!("Failed to parse LLM questions for room {}: {}. Using default questions. LLM response was: {}", room_name, e, json_text);
                                create_questions()
                            }
                        }
                    }
                    Err(e) => {
                        info!("Failed to generate LLM questions for room {}: {}. Using default questions.", room_name, e);
                        create_questions()
                    }
                };
                
                // Reset the game state and set new questions
                let mut game = game_data.lock().unwrap();
                game.state = GameState::WaitingForPlayers;
                game.current_results.clear();
                game.questions = questions_to_use;
                game.questions_ready = true; // Questions are ready since we just generated them
                // Reset all players' scores and states
                for player in game.players.values_mut() {
                    player.score = 0;
                    player.current_answer = None;
                    player.ready_for_next = false;
                    player.is_ready_to_start = false;
                }
                info!("Room {} reset for new game as player {} joined.", room_name, player_name);
            }

            let mut game = game_data.lock().unwrap();
            let mut existing_player_id_to_remove: Option<String> = None;
            for p in game.players.values() {
                if p.name == player_name {
                    existing_player_id_to_remove = Some(p.id.clone());
                    break;
                }
            }

            if let Some(id_to_remove) = existing_player_id_to_remove {
                game.players.remove(&id_to_remove);
                info!("Player {} rejoining room {}, replacing old entry.", player_name, room_name);
            }

            let is_host = game.players.is_empty();
            let new_player_id = Uuid::new_v4().to_string();
            let player = Player {
                id: new_player_id.clone(),
                name: player_name.clone(),
                score: 0, 
                current_answer: None,
                ready_for_next: false,
                is_ready_to_start: false, 
                is_host,
            };

            game.players.insert(player.id.clone(), player.clone());
            
            let _ = game.tx.send(GameMessage::PlayerJoined { player: player.clone() });

            Ok(JoinRoomResponse {
                room_id: room_name.clone(), 
                room_name,
                player,
                is_host,
            })
        }
        _ => {
            info!("Player {} attempted to join room {} which is in progress.", player_name, room_name);
            Err("Game in progress".to_string())
        }
    }
}

pub async fn handle_return_to_lobby(
    game_data: &Arc<Mutex<GameData>>, 
    player_id: &str,
    room_name: &str
) -> Result<(), String> {
    // Check if the requesting player is the host
    {
        let game = game_data.lock().unwrap();
        if let Some(player) = game.players.get(player_id) {
            if !player.is_host {
                return Err("Only the host can return to lobby".to_string());
            }
        } else {
            return Err("Player not found".to_string());
        }
    }
    
    // Reset the game state immediately
    {
        let mut game = game_data.lock().unwrap();
        game.state = GameState::WaitingForPlayers;
        game.questions = create_questions(); // Use default questions temporarily
        game.questions_ready = false; // Mark as not ready until new questions are generated
        game.current_results.clear();
        
        // Reset all players' game state but keep them in the room
        for player in game.players.values_mut() {
            player.score = 0;
            player.current_answer = None;
            player.ready_for_next = false;
            player.is_ready_to_start = false;
        }
        
        // Notify all players that they've returned to lobby
        let _ = game.tx.send(GameMessage::ReturnedToLobby);
        
        info!("Room {} has been reset to lobby with {} players. Starting background question generation.", room_name, game.players.len());
    }
    
    // Start generating new questions asynchronously
    let game_data_clone = game_data.clone();
    let room_name_clone = room_name.to_string();
    tokio::spawn(async move {
        generate_questions_async(game_data_clone, room_name_clone).await;
    });
    
    Ok(())
} 