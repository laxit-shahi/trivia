use reqwest::Client;
use serde_json::json;
use serde::{Deserialize, Serialize};
use std::env;

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

fn get_api_token() -> Result<String, Box<dyn std::error::Error>> {
    env::var("SHOPIFY_API_TOKEN")
        .map_err(|_| "SHOPIFY_API_TOKEN environment variable not set".into())
}

pub async fn generate_trivia_questions() -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_token = get_api_token()?;
    
    let instructions = "You are a super amazing life chaning trivia generator that is 9/10 sassy. 10 being the sassiest person in the world, 0being not sassy at all.";
    let category = "General";
    let difficulty = 5;
    let num_of_questions = 2;
    let age_group = "22-50";
    let hint_level = "0";
    
    let trivia_prompt = format!(
        "Create a set of trivia questions. Each of the questions should be of a difficult out of ten, in the sense that in a random sample of 10 people, 3 of the people in the room should know the answer. Hint level should be out of 10. O being not hint and 10 being a super big hint. Give me the answer below the hint line. I will give you further detail on specfic details below. Category: {}, Number of questions: {}, Difficulty: {} , Age Group: {}, hints: {} --- Format: {{\"questions\":[{{\"question\":\"The text for question one\", \"answer\": \"Answer to question\", \"hint\":\"Hint to corresponding question\"}}, {{\"question\":\"...\"}}, {{\"question\":\"...\"}}]}}. No spacing between, just normal json format so i can run json.parse() on it.",
        category, num_of_questions, difficulty, age_group, hint_level
    );
    
    let payload = json!({
        "model": "expensive-but-best",
        "instructions": instructions,
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
    
    Err("Could not extract text from response".into())
}

pub async fn check_answer_correctness(
    question: &str,
    correct_answer: &str,
    player_answer: &str,
) -> Result<AnswerCorrectness, Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_token = get_api_token()?;
    
    let prompt = format!(
        "You are an expert trivia judge. Evaluate if a player's answer is correct, partially correct, or wrong.\n\nQuestion: {}\nCorrect Answer: {}\nPlayer Answer: {}\n\nReturn ONLY one word: 'CORRECT' if the answer is exactly right or equivalent, 'PARTIAL' if it's close but missing something important, or 'WRONG' if it's completely incorrect.\n\nConsider synonyms, alternate spellings, and reasonable interpretations as correct. Consider answers that capture the main idea but lack precision as partial.",
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