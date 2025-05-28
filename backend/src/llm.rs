use reqwest::Client;
use serde_json::json;

pub async fn generate_trivia_questions() -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    
    let instructions = "You are a super amazing life chaning trivia generator that is 9/10 sassy. 10 being the sassiest person in the world, 0being not sassy at all.";
    let category = "General";
    let difficulty = 10;
    let num_of_questions = 10;
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
        .header("Authorization", "Bearer shopify-eyJpZCI6IjVhYjEwMzdiZjE2ODc1NjcyMTc4ZjJhYWY5ZGI2M2FhIiwibW9kZSI6InBlcnNvbmFsIiwiZW1haWwiOiJsYXhpdC5zaGFoaUBzaG9waWZ5LmNvbSIsImV4cGlyeSI6MTc0ODUzNDU2OX0=-P6eMEFXNtyZ6JG88tDj+YREe2FjkbGzFvzX5QxSriQE=")
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trivia_text = generate_trivia_questions().await?;
    println!("{}", trivia_text);
    Ok(())
}