# 🧠 Trivia Game

A real-time multiplayer trivia game built with React TypeScript frontend and Rust Axum backend. Players can join a game hosted on one machine and play together in real-time.

## Features

- **Real-time multiplayer**: Multiple players can join and play simultaneously
- **WebSocket communication**: Instant updates for all players
- **10 trivia questions**: Static set of questions covering various topics
- **Score tracking**: Automatic scoring and final leaderboard
- **Modern UI**: Beautiful, responsive design with smooth animations
- **Host controls**: First player becomes the host and can start the game

## Tech Stack

### Backend
- **Rust** with **Axum** web framework
- **WebSocket** support for real-time communication
- **Tokio** for async runtime
- **Serde** for JSON serialization
- **CORS** enabled for cross-origin requests

### Frontend
- **React** with **TypeScript**
- **Axios** for HTTP requests
- **WebSocket** for real-time updates
- **Modern CSS** with gradients and animations

## Setup Instructions

### Prerequisites
- **Rust** (latest stable version)
- **Node.js** (v16 or higher)
- **npm** or **yarn**

### Backend Setup

1. Navigate to the backend directory:
```bash
cd backend
```

2. Install dependencies and run the server:
```bash
cargo run
```

The backend server will start on `http://localhost:3001`

### Frontend Setup

1. Navigate to the frontend directory:
```bash
cd frontend
```

2. Install dependencies:
```bash
npm install
```

3. Start the development server:
```bash
npm start
```

The frontend will start on `http://localhost:3000`

## How to Play

### For the Host (First Player)

1. Open your browser and go to `http://localhost:3000`
2. Enter your name and click "Join Game"
3. Wait for other players to join
4. Click "Start Game" when ready

### For Other Players

1. Open your browser and go to `http://<HOST_IP>:3000` (replace `<HOST_IP>` with the host's IP address)
2. Enter your name and click "Join Game"
3. Wait for the host to start the game

### Game Flow

1. **Joining**: Players enter their names and join the game
2. **Waiting**: All players wait in a lobby until the host starts the game
3. **Questions**: 10 questions are presented one by one
4. **Answering**: Players type their answers and submit them
5. **Results**: After everyone submits, results are shown with correct/incorrect indicators
6. **Next Question**: Players click "Ready for Next Question" to continue
7. **Final Scores**: After 10 questions, final scores and rankings are displayed

## Game Rules

- **Exact Match**: Answers must exactly match the correct answer (case-insensitive)
- **All Players**: Everyone must submit an answer before results are shown
- **Ready Check**: All players must click "Ready for Next Question" to proceed
- **Scoring**: 1 point per correct answer, maximum 10 points

## Sample Questions

The game includes 10 built-in questions covering:
- Geography (capitals, countries, rivers)
- Mathematics (basic arithmetic)
- Science (planets, chemistry, biology)
- History (World War II)
- Art (famous paintings)
- General knowledge

## Network Setup

### For Local Network Play

1. Find the host machine's IP address:
   - **Windows**: `ipconfig`
   - **macOS/Linux**: `ifconfig` or `ip addr`

2. Other players connect to: `http://<HOST_IP>:3000`

3. Ensure firewall allows connections on port 3000

### Port Configuration

- **Backend**: Port 3001 (API and WebSocket)
- **Frontend**: Port 3000 (React dev server)

## Development

### Backend Development

```bash
cd backend
cargo watch -x run  # Auto-reload on changes
```

### Frontend Development

```bash
cd frontend
npm start  # Auto-reload on changes
```

### Adding New Questions

Edit the `create_questions()` function in `backend/src/main.rs`:

```rust
fn create_questions() -> Vec<Question> {
    vec![
        Question {
            question: "Your question here?".to_string(),
            answer: "Your answer here".to_string(),
        },
        // Add more questions...
    ]
}
```

## API Endpoints

- `POST /api/join` - Join the game
- `POST /api/start` - Start the game (host only)
- `POST /api/submit-answer` - Submit an answer
- `POST /api/ready-next` - Mark ready for next question
- `GET /ws` - WebSocket connection for real-time updates

## Troubleshooting

### Common Issues

1. **WebSocket connection failed**
   - Check if backend is running on port 3001
   - Verify firewall settings

2. **Players can't join**
   - Ensure all players use the correct IP address
   - Check network connectivity

3. **Game doesn't start**
   - Only the first player (host) can start the game
   - Ensure at least one player has joined

### Logs

- **Backend logs**: Check the terminal running `cargo run`
- **Frontend logs**: Check browser developer console (F12)

## License

This project is open source and available under the MIT License. 