use crate::types::Question;
use rand::seq::SliceRandom;

pub fn create_questions() -> Vec<Question> {
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

pub fn parse_generated_questions(json_text: &str) -> Result<Vec<Question>, Box<dyn std::error::Error>> {
    let parsed: serde_json::Value = serde_json::from_str(json_text)?;
    
    let questions_array = parsed.get("questions")
        .and_then(|q| q.as_array())
        .ok_or("No 'questions' array found in response")?;
    
    let mut questions = Vec::new();
    for question_obj in questions_array {
        let question_text = question_obj.get("question")
            .and_then(|q| q.as_str())
            .ok_or("Missing or invalid 'question' field")?;
        
        let answer_text = question_obj.get("answer")
            .and_then(|a| a.as_str())
            .ok_or("Missing or invalid 'answer' field")?;
        
        questions.push(Question {
            question: question_text.to_string(),
            answer: answer_text.to_string(),
        });
    }
    
    if questions.is_empty() {
        return Err("No valid questions found in response".into());
    }
    
    Ok(questions)
}

pub fn generate_room_name() -> String {
    let adjectives = [
        "Happy", "Clever", "Brave", "Swift", "Bright", "Calm", "Epic", "Wild",
        "Cool", "Smart", "Quick", "Strong", "Lucky", "Bold", "Fast", "Wise",
    ];
    let animals = [
        "Tiger", "Eagle", "Dolphin", "Lion", "Fox", "Wolf", "Bear", "Hawk",
        "Shark", "Panther", "Falcon", "Jaguar", "Lynx", "Orca", "Raven", "Cobra",
    ];

    let mut rng = rand::thread_rng();
    let adjective = adjectives.choose(&mut rng).unwrap();
    let animal = animals.choose(&mut rng).unwrap();
    format!("{}{}", adjective, animal)
} 