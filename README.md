# 🎯 Trivia Game

A real-time multiplayer trivia game with AI-generated questions, featuring a sassy AI personality and comprehensive question logging to avoid duplicates.

## 🚀 Quick Start

### One Script Does Everything!

#### Local Network Access (WiFi sharing)
```bash
./start-trivia.sh
```
- Starts both backend and frontend servers
- Exposes them on your network (0.0.0.0)
- Shows your IP address to share with friends
- Your computer becomes the game server

#### Public Internet Access (ngrok tunnels)
```bash
./start-trivia.sh --ngrok
```
- Does everything above PLUS
- Creates public ngrok tunnels
- Anyone on the internet can join
- Shows public URLs to share

### That's it! 🎉
- Players join by visiting the game URL
- No room codes needed - automatic room system
- Press Ctrl+C to stop everything

## 📋 Prerequisites

- **Rust & Cargo**: Install from [rustup.rs](https://rustup.rs/)
- **Node.js & npm**: Install from [nodejs.org](https://nodejs.org/)
- **ngrok** (optional): Install with `brew install ngrok` or from [ngrok.com](https://ngrok.com/)

## 🎮 How to Play

1. **Start the app** using one of the commands above
2. **Share the frontend URL** with friends
3. **Join the room** - players automatically join the main room
4. **Mark as ready** when all players have joined
5. **Answer questions** as they appear
6. **View results** after each question
7. **See final scores** at the end

## ⚙️ Game Settings

The lobby displays current game settings:
- **Category**: Food and Drinks
- **Difficulty**: 10/10 (Expert level)
- **Questions**: 10 per game
- **Age Group**: 22-50
- **Hint Level**: 0/10 (No hints)
- **AI Personality**: 9/10 Sassy

## 🌐 Access Options Explained

### Local Only (`./start-trivia.sh`)
- Frontend: `http://localhost:3000`
- Backend: `http://localhost:3001`
- **Who can access**: Only you on your computer

### Network Access (`./start-trivia.sh --network`)
- Frontend: `http://[YOUR-IP]:3000`
- Backend: `http://[YOUR-IP]:3001`
- **Who can access**: Anyone on your local network (WiFi/LAN)
- **Find your IP**: `ifconfig` (macOS/Linux) or `ipconfig` (Windows)

### Public Access (`./start-trivia.sh --ngrok`)
- Frontend: `https://[random].ngrok.io`
- Backend: `https://[random].ngrok.io`
- **Who can access**: Anyone on the internet with the URL
- **View URLs**: Check `http://localhost:4040` for ngrok dashboard

## 🛠️ Manual Setup (Alternative)

If you prefer to start services manually:

### Backend
```bash
cd backend
# For local development only
SHOPIFY_API_TOKEN=dummy cargo run

# For network access (others can join)  
HOST=0.0.0.0 SHOPIFY_API_TOKEN=dummy cargo run
```

### Frontend
```bash
cd frontend
# For local development
npm start

# For network access
HOST=0.0.0.0 npm start

# For ngrok (after getting backend URL)
REACT_APP_API_BASE_URL=https://your-backend.ngrok.io/api \
REACT_APP_WS_URL=wss://your-backend.ngrok.io/ws \
npm run build && npx serve -s build -l 3000
```

## 🔧 Troubleshooting

### Ngrok Issues
- Install ngrok: `brew install ngrok`
- Sign up at ngrok.com for better reliability
- Check ngrok dashboards at `http://localhost:4040` and `http://localhost:4041`

### Network Access Not Working
- Make sure your firewall allows connections on ports 3000 and 3001
- Other devices must be on the same network (WiFi/LAN)
- Try accessing `http://[your-ip]:3000` from another device

## 🎯 Game Features

- **Real-time Multiplayer**: Join games instantly, see live updates
- **AI-Generated Questions**: Unique questions every game with smart duplicate prevention
- **Sassy AI Personality**: 9/10 sassy trivia host for entertaining questions
- **Question Logging**: All questions logged with timestamps to prevent repeats
- **Responsive Design**: Works on desktop, tablet, and mobile
- **Single Room System**: Simplified joining - no room codes needed
- **Live Scoring**: Real-time score updates and final leaderboards
- **Victory Badges**: Download celebration badges for top 3 finishers

## 🔄 Stopping the Server

Press `Ctrl+C` in the terminal running the script. It automatically:
- Stops both frontend and backend servers
- Closes any ngrok tunnels
- Cleans up processes and ports

## 📝 Technical Notes

- Backend uses a dummy API token for development
- Questions are logged to `backend/questions.json`
- Settings can be modified in `backend/src/llm.rs`
- The app automatically handles player reconnections