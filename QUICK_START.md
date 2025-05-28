# 🚀 Quick Start Guide

## Start the Game (2 Terminal Windows Required)

### Terminal 1 - Backend Server
```bash
./start-backend.sh
```
Or manually:
```bash
cd backend
cargo run
```

### Terminal 2 - Frontend Server
```bash
./start-frontend.sh
```
Or manually:
```bash
cd frontend
npm start
```

## Play the Game

1. **Host**: Open `http://localhost:3000` in your browser
2. **Other Players**: Open `http://<HOST_IP>:3000` in their browsers
3. **Everyone**: Enter your name and click "Join Game"
4. **Host**: Click "Start Game" when everyone has joined
5. **Play**: Answer 10 trivia questions together!

## Game Features

✅ **Real-time multiplayer** - Everyone sees updates instantly  
✅ **10 trivia questions** - Geography, science, history, and more  
✅ **Automatic scoring** - Case-insensitive answer matching  
✅ **Beautiful UI** - Modern design with smooth animations  
✅ **Host controls** - First player becomes the host  
✅ **Final leaderboard** - See who won at the end  

## Network Setup for Multiple Machines

1. Find your IP address:
   - **macOS/Linux**: `ifconfig | grep inet`
   - **Windows**: `ipconfig`

2. Share your IP with other players
3. They visit: `http://YOUR_IP:3000`

## Troubleshooting

- **Backend not starting**: Make sure Rust is installed (`rustup update`)
- **Frontend not starting**: Make sure Node.js is installed (`node --version`)
- **Can't connect**: Check firewall settings for ports 3000 and 3001
- **WebSocket errors**: Ensure both servers are running

## Sample Questions Included

1. What is the capital of France?
2. What is 2 + 2?
3. What is the largest planet in our solar system?
4. Who painted the Mona Lisa?
5. What is the chemical symbol for gold?
6. In which year did World War II end?
7. What is the smallest country in the world?
8. How many continents are there?
9. What is the longest river in the world?
10. What gas do plants absorb from the atmosphere?

**Have fun playing trivia! 🧠🎉** 