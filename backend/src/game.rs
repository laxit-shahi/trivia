use crate::types::*;
use crate::utils::{create_questions, parse_generated_questions};
use crate::llm;
use crate::log_questions_to_file;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;
use crate::{Player, GameState, GameData, GameMessage, Question, PlayerResult, CreateRoomResponse, JoinRoomResponse};

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

        if all_players_ready && enough_players && current_game_state == GameState::WaitingForPlayers {
            initiate_game_start_sequence = true;
            info!("All players ready in room {}. Initiating LLM question generation.", room_name);
        }
    }

    if initiate_game_start_sequence {
        start_game_sequence(game_data_arc, &room_name_clone).await?;
    }

    Ok(())
}

async fn start_game_sequence(game_data_arc: Arc<Mutex<GameData>>, room_name: &str) -> Result<(), String> {
    // Generate questions asynchronously
    let generated_questions_result = llm::generate_trivia_questions().await;

    let questions_to_use = match generated_questions_result {
        Ok(json_text) => {
            match parse_generated_questions(&json_text) {
                Ok(parsed_q) if !parsed_q.is_empty() => {
                    info!("Successfully parsed {} questions from LLM for room {}", parsed_q.len(), room_name);
                    parsed_q
                }
                Ok(_) => {
                    info!("LLM returned empty question set for room {}, falling back to default questions.", room_name);
                    create_questions() 
                }
                Err(e) => {
                    info!("Failed to parse LLM questions for room {}: {}. Falling back to default questions.", room_name, e);
                    create_questions()
                }
            }
        }
        Err(e) => {
            info!("Failed to generate LLM questions for room {}: {}. Falling back to default questions.", room_name, e);
            create_questions()
        }
    };

    // Update game state with questions and send start messages
    {
        let mut game = game_data_arc.lock().unwrap();
        if game.state == GameState::WaitingForPlayers { 
            game.state = GameState::InProgress { current_question: 0 };
            game.questions = questions_to_use.clone();
            
            // Log questions to file
            log_questions_to_file(&questions_to_use, "General", room_name);
            
            let total_questions = game.questions.len() as u32;
            info!("Game starting in room {} with {} players, using {} questions.", 
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
            } else {
                info!("Error: No questions available to start game in room {}. Reverting state.", room_name);
                game.state = GameState::WaitingForPlayers; 
                let _ = game.tx.send(GameMessage::Error { message: "Failed to load questions for game start.".to_string() });
                return Err("No questions available".to_string());
            }
        } else {
             info!("Game in room {} was already started. LLM questions were fetched but not used.", room_name);
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
                        // Reset ready_for_next for all players
                        for p in game.players.values_mut() {
                            p.ready_for_next = false;
                        }
                    }
                }
            }
        }
        
        Ok(())
    } else {
        Err("Player not found".to_string())
    }
}

pub fn create_new_room(room_name: String, player_name: String) -> (Arc<Mutex<GameData>>, CreateRoomResponse) {
    let room_id = Uuid::new_v4().to_string();
    let (tx, _rx) = broadcast::channel(100);
    
    let player = Player {
        id: Uuid::new_v4().to_string(),
        name: player_name.clone(),
        score: 0,
        current_answer: None,
        ready_for_next: false,
        is_ready_to_start: false,
    };
    
    let mut players = HashMap::new();
    players.insert(player.id.clone(), player.clone());
    
    let game_data = GameData {
        players,
        state: GameState::WaitingForPlayers,
        questions: Vec::new(),
        current_results: Vec::new(),
        tx,
    };
    
    let game_data_arc = Arc::new(Mutex::new(game_data));
    
    // Send player joined message
    {
        let game = game_data_arc.lock().unwrap();
        let _ = game.tx.send(GameMessage::PlayerJoined { player: player.clone() });
    }
    
    let response = CreateRoomResponse {
        room_id,
        room_name: room_name.clone(),
        player,
        is_host: true,
    };
    
    (game_data_arc, response)
}

pub fn join_existing_room(
    game_data: &Arc<Mutex<GameData>>, 
    player_name: String, 
    room_name: String
) -> Result<JoinRoomResponse, String> {
    let mut game = game_data.lock().unwrap();

    match game.state {
        GameState::WaitingForPlayers | GameState::Ended => {
            // If game has ended, reset it to waiting for players
            if game.state == GameState::Ended {
                game.state = GameState::WaitingForPlayers;
                game.questions.clear();
                game.current_results.clear();
                // Reset all players' scores and states
                for player in game.players.values_mut() {
                    player.score = 0;
                    player.current_answer = None;
                    player.ready_for_next = false;
                    player.is_ready_to_start = false;
                }
                info!("Room {} reset for new game as player {} joined.", room_name, player_name);
            }

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