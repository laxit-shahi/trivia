use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Scoreboard {
    scores: HashMap<String, i32>,
}

impl Scoreboard {
    pub fn new() -> Self {
        Scoreboard {
            scores: HashMap::new(),
        }
    }

    pub fn add_player(&mut self, player_name: &str) {
        self.scores.entry(player_name.to_string()).or_insert(0);
    }

    pub fn update_score(&mut self, player_name: &str, points_change: i32) {
        let score = self.scores.entry(player_name.to_string()).or_insert(0);
        *score += points_change;
    }

    pub fn get_player_score(&self, player_name: &str) -> Option<i32> {
        self.scores.get(player_name).copied()
    }

    pub fn get_all_scores(&self) -> Vec<(String, i32)> {
        let mut sorted_scores: Vec<(String, i32)> = self.scores.iter()
            .map(|(name, score)| (name.clone(), *score))
            .collect();
        
        sorted_scores.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        
        sorted_scores
    }

    pub fn reset(&mut self) {
        self.scores.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_scoreboard_is_empty() {
        let sb = Scoreboard::new();
        assert_eq!(sb.get_all_scores().len(), 0);
    }

    #[test]
    fn test_add_player() {
        let mut sb = Scoreboard::new();
        sb.add_player("Alice");
        assert_eq!(sb.get_player_score("Alice"), Some(0));
        assert_eq!(sb.get_all_scores(), vec![("Alice".to_string(), 0)]);
    }

    #[test]
    fn test_add_existing_player() {
        let mut sb = Scoreboard::new();
        sb.add_player("Alice");
        sb.update_score("Alice", 10);
        sb.add_player("Alice"); 
        assert_eq!(sb.get_player_score("Alice"), Some(10));
    }

    #[test]
    fn test_update_score_new_player() {
        let mut sb = Scoreboard::new();
        sb.update_score("Bob", 50);
        assert_eq!(sb.get_player_score("Bob"), Some(50));
        assert_eq!(sb.get_all_scores(), vec![("Bob".to_string(), 50)]);
    }

    #[test]
    fn test_update_score_existing_player() {
        let mut sb = Scoreboard::new();
        sb.add_player("Carol");
        sb.update_score("Carol", 100);
        sb.update_score("Carol", 25);
        assert_eq!(sb.get_player_score("Carol"), Some(125));
    }

    #[test]
    fn test_get_player_score_non_existent() {
        let sb = Scoreboard::new();
        assert_eq!(sb.get_player_score("Dave"), None);
    }

    #[test]
    fn test_get_all_scores_sorted() {
        let mut sb = Scoreboard::new();
        sb.update_score("Eve", 70);
        sb.update_score("Frank", 90);
        sb.update_score("Grace", 70); 

        let expected_sorted_scores = vec![
            ("Frank".to_string(), 90),
            ("Eve".to_string(), 70),
            ("Grace".to_string(), 70),
        ];

        assert_eq!(sb.get_all_scores(), expected_sorted_scores);
    }
    
    #[test]
    fn test_get_all_scores_empty() {
        let sb = Scoreboard::new();
        assert_eq!(sb.get_all_scores().len(), 0);
    }

    #[test]
    fn test_reset_scoreboard() {
        let mut sb = Scoreboard::new();
        sb.add_player("Hannah");
        sb.update_score("Hannah", 200);
        sb.reset();
        assert_eq!(sb.get_player_score("Hannah"), None);
        assert_eq!(sb.get_all_scores().len(), 0);
    }
} 