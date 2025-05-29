use crate::llm::{Question, AnswerCorrectness, check_answer_correctness};
use crate::scoreboard::Scoreboard;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameState {
    Pending, // Waiting for players or setup
    InProgress, // Questions are being asked
    RoundOver, // All questions for the current round asked
    Finished,  // Game complete, final scores available
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    id: String, // Unique identifier for the player
    // Add other player-specific info if needed, e.g., name, connection_id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String, // Unique identifier for the game
    pub questions: Vec<Question>,
    pub current_question_index: usize,
    pub players: HashMap<String, Player>, // Player ID to Player struct
    pub scoreboard: Scoreboard,
    pub state: GameState,
    // Game settings
    pub category: String,
    pub difficulty: u8,
    pub num_of_questions: u32,
    pub age_group: String,
    pub hint_level: u8,
}

impl Game {
    pub fn new(
        id: String,
        questions: Vec<Question>,
        category: String,
        difficulty: u8,
        num_of_questions: u32,
        age_group: String,
        hint_level: u8,
    ) -> Self {
        Game {
            id,
            questions,
            current_question_index: 0,
            players: HashMap::new(),
            scoreboard: Scoreboard::new(),
            state: GameState::Pending,
            category,
            difficulty,
            num_of_questions,
            age_group,
            hint_level,
        }
    }

    pub fn add_player(&mut self, player_id: String) -> Result<(), String> {
        if self.players.contains_key(&player_id) {
            return Err(format!("Player {} already in game {}", player_id, self.id));
        }
        self.players.insert(player_id.clone(), Player { id: player_id.clone() });
        self.scoreboard.add_player(&player_id);
        Ok(())
    }

    pub fn start_game(&mut self) -> Result<(), String> {
        if self.questions.is_empty() {
            return Err("Cannot start game: no questions loaded".to_string());
        }
        if self.players.is_empty() {
            return Err("Cannot start game: no players have joined".to_string());
        }
        self.state = GameState::InProgress;
        self.current_question_index = 0;
        Ok(())
    }

    pub fn get_current_question(&self) -> Option<&Question> {
        if self.state != GameState::InProgress && self.state != GameState::RoundOver {
            return None; // Or handle error, game not in a state to provide questions
        }
        self.questions.get(self.current_question_index)
    }

    pub async fn handle_submission(
        game_arc: Arc<RwLock<Game>>,
        player_id: &str,
        answer: &str,
    ) -> Result<AnswerCorrectness, String> {
        let game_read_guard = game_arc.read().await;
        if game_read_guard.state != GameState::InProgress {
            return Err("Game is not in progress. Cannot submit answer.".to_string());
        }

        let question = game_read_guard.get_current_question()
            .ok_or_else(|| "No current question available".to_string())?;

        if !game_read_guard.players.contains_key(player_id) {
            return Err(format!("Player {} not found in game {}", player_id, game_read_guard.id));
        }
        
        let correctness = check_answer_correctness(&question.question, &question.answer, answer)
            .await
            .map_err(|e| format!("Error checking answer: {}", e))?;
        
        drop(game_read_guard); // Release read lock

        let mut game_write_guard = game_arc.write().await;
        match correctness {
            AnswerCorrectness::Correct => game_write_guard.scoreboard.increment_score(player_id, 10),
            AnswerCorrectness::Partial => game_write_guard.scoreboard.increment_score(player_id, 5),
            AnswerCorrectness::Wrong => { /* No points for wrong answer */ }
        }

        Ok(correctness)
    }

    pub fn next_question(&mut self) -> Result<Option<&Question>, String> {
        if self.state != GameState::InProgress {
            return Err("Game is not in progress. Cannot advance to next question.".to_string());
        }
        if self.current_question_index < self.questions.len() - 1 {
            self.current_question_index += 1;
            Ok(self.questions.get(self.current_question_index))
        } else {
            self.state = GameState::RoundOver; // Or GameState::Finished if it's the end of the game
            Ok(None) // No more questions
        }
    }
    
    pub fn end_round(&mut self) {
        // Potentially move to GameState::Finished if there's only one round
        // Or prepare for a new round if multiple rounds are supported
        self.state = GameState::Finished; 
        // In a multi-round game, this might be GameState::RoundOver
        // and another function would start_new_round()
    }

    pub fn get_game_summary(&self) -> HashMap<String, i32> {
        self.scoreboard.get_all_scores().into_iter().collect()
    }
} 