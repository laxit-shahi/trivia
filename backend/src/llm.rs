use reqwest::Client;
use serde_json::json;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;
use crate::types::TriviaSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnswerCorrectness {
    Correct,
    Partial, 
    Wrong,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoggedQuestion {
    question: String,
    answer: String,
    category: String,
    timestamp: String,
    room_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct QuestionsLog {
    questions: Vec<LoggedQuestion>,
}

fn get_api_token() -> Result<String, Box<dyn std::error::Error>> {
    env::var("SHOPIFY_API_TOKEN")
        .map_err(|_| "SHOPIFY_API_TOKEN environment variable not set".into())
}

fn read_existing_questions() -> Vec<String> {
    let questions_file = "questions.json";
    
    if !Path::new(questions_file).exists() {
        return Vec::new();
    }
    
    match fs::read_to_string(questions_file) {
        Ok(content) => {
            match serde_json::from_str::<QuestionsLog>(&content) {
                Ok(log) => {
                    log.questions.into_iter().map(|q| q.question).collect()
                }
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    }
}

pub async fn generate_trivia_questions() -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_token = get_api_token()?;
    
    // Debug: Print token info (first and last few characters for security)
    println!("Using API token: {}...{}", 
             &api_token[..std::cmp::min(20, api_token.len())], 
             &api_token[std::cmp::max(0, api_token.len().saturating_sub(10))..]);
    
    // Read existing questions to avoid duplicates
    let existing_questions = read_existing_questions();
    
    // Get current trivia settings
    let settings = get_trivia_settings();
    
    // Build the exclusion part of the prompt
    let exclusion_text = if existing_questions.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nIMPORTANT: DO NOT use any of these previously used questions or create variations of them:\n{}\n\nMake sure your new questions are completely different and original.",
            existing_questions.iter()
                .enumerate()
                .map(|(i, q)| format!("{}. {}", i + 1, q))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    
    let trivia_prompt = format!(
        "Create a set of trivia questions. Each of the questions should be of a difficulty out of ten. If it is 3/10 it would mean that in a random sample of 10 people, 7 of the people in the room should know the answer. If it's 9/10, only 1 person should know the answer on average. Hint level should be out of 10. This means that 1 is the easiest, 10 is the hardest --- O being not hint and 10 being a super big hint. Give me the answer below the hint line. I will give you further specific details below. Category: {}, Number of questions: {}, Difficulty: {} , Age Group: {}, hints: {}{} --- Format: {{\"questions\":[{{\"question\":\"The text for question one\", \"answer\": \"Answer to question\", \"hint\":\"Hint to corresponding question\"}}, {{\"question\":\"...\"}}, {{\"question\":\"...\"}}]}}. No spacing between, just normal json format so i can run json.parse() on it.",
        settings.category, settings.num_of_questions, settings.difficulty, settings.age_group, settings.hint_level, exclusion_text
    );
    
    let payload = json!({
        "model": "expensive-but-best",
        "instructions": settings.instructions,
        "input": trivia_prompt
    });
    
    let response = client
        .post("https://proxy.shopify.ai/v1/responses")
        .header("Authorization", format!("Bearer {}", api_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;
    
    let response_json: serde_json::Value = response.json().await?;
    
    // Save the entire response to output.json
    std::fs::write("output.json", serde_json::to_string_pretty(&response_json)?)?;
    
    // Extract just the text content from output[0].content[0].text
    if let Some(output) = response_json.get("output") {
        if let Some(first_output) = output.get(0) {
            if let Some(content) = first_output.get("content") {
                if let Some(first_content) = content.get(0) {
                    if let Some(text) = first_content.get("text") {
                        return Ok(text.as_str().unwrap_or("").to_string());
                    }
                }
            }
        }
    }
    
    Err(format!("Could not extract text from response. Full response: {}", 
        serde_json::to_string_pretty(&response_json).unwrap_or_else(|_| "Failed to serialize response".to_string())
    ).into())
}

pub async fn check_answer_correctness(
    question: &str,
    correct_answer: &str,
    player_answer: &str,
) -> Result<AnswerCorrectness, Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_token = get_api_token()?;
    
    let prompt = format!(
        "You are an expert trivia judge. Evaluate if a player's answer is correct, partially correct, or wrong. 'CORRECT' means that the answer is exactly right or equivalent, 'PARTIAL' if it's close but missing something important, or 'WRONG' if it's completely incorrect. If the answer is close but missing something important, it is partially correct. Typos and small differences in the answer are allowed and should still be correct. \n\nQuestion: {}\nCorrect Answer: {}\nPlayer Answer: {}\n\nReturn ONLY one word: 'CORRECT', 'PARTIAL', or 'WRONG'.\n\nConsider synonyms, alternate spellings, and reasonable interpretations as correct. Consider answers that capture the main idea but lack precision as partial.",
        question, correct_answer, player_answer
    );
    
    let payload = json!({
        "model": "expensive-but-best",
        "instructions": "You are a precise trivia judge. Return only CORRECT, PARTIAL, or WRONG.",
        "input": prompt
    });
    
    let response = client
        .post("https://proxy.shopify.ai/v1/responses")
        .header("Authorization", format!("Bearer {}", api_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;
    
    let response_json: serde_json::Value = response.json().await?;
    
    // Extract the text response
    let result_text = if let Some(output) = response_json.get("output") {
        if let Some(first_output) = output.get(0) {
            if let Some(content) = first_output.get("content") {
                if let Some(first_content) = content.get(0) {
                    if let Some(text) = first_content.get("text") {
                        text.as_str().unwrap_or("").to_uppercase().trim().to_string()
                    } else {
                        return Err("No text in response".into());
                    }
                } else {
                    return Err("No content in response".into());
                }
            } else {
                return Err("No content array in response".into());
            }
        } else {
            return Err("No first output in response".into());
        }
    } else {
        return Err("No output in response".into());
    };
    
    match result_text.as_str() {
        "CORRECT" => Ok(AnswerCorrectness::Correct),
        "PARTIAL" => Ok(AnswerCorrectness::Partial),
        "WRONG" => Ok(AnswerCorrectness::Wrong),
        _ => {
            // Fallback to exact match if LLM response is unclear
            if player_answer.trim().eq_ignore_ascii_case(correct_answer.trim()) {
                Ok(AnswerCorrectness::Correct)
            } else {
                Ok(AnswerCorrectness::Wrong)
            }
        }
    }
}

pub fn get_trivia_settings() -> TriviaSettings {
    TriviaSettings {
        instructions: [
            "You are the best and most loved bar trivia runner in the world.",
            "You generate questions that are appropriate for bar trivia.",
            "You review each question after generating it to check for inaccuracies or obvious hints.",
            "For example, you wouldn't use a clue like \"What is the term for when water is displaced by an object floating in it\" if the answer is Displacement.",
            "You also wouldn't have a question like \"What element is named after the city Marie Curie is from?\" where the answer is Polonium, because Poland is a country, not a city."
        ].join(" ").to_string(),
        category: "General Bar trivia. This means a variety of categories enjoyable to people of all walks of life. This should be intellectual categories, and pop culture and music stuff.".to_string(),
        difficulty: 5,
        num_of_questions: 10,
        age_group: "22-50".to_string(),
        hint_level: "0".to_string(),
    }
}