import React, { useState, useEffect, useCallback, useRef } from 'react';
import { BrowserRouter as Router, Routes, Route, useParams, useNavigate } from 'react-router-dom';
import axios from 'axios';
import './App.css';

interface Player {
  id: string;
  name: string;
  score: number;
  current_answer?: string;
  ready_for_next: boolean;
  is_ready_to_start: boolean;
}

interface PlayerResult {
  player_name: string;
  answer: string;
  correctness: 'Correct' | 'Partial' | 'Wrong';
  correct_answer: string;
  has_skipped_voting?: boolean;
}

interface GameMessage {
  type: string;
  player?: Player;
  is_ready?: boolean;
  num_questions?: number;
  question?: string;
  question_number?: number;
  player_id?: string;
  answer?: string;
  results?: PlayerResult[];
  correct_answer?: string;
  voter_id?: string;
  target_player_id?: string;
  updated_results?: PlayerResult[];
  final_scores?: Player[];
  message?: string;
  player_id_skipped?: string;
}

enum GameState {
  HOME = 'home',
  JOIN_ROOM = 'join_room',
  WAITING = 'waiting',
  QUESTION = 'question',
  RESULTS = 'results',
  FINAL_SCORES = 'final_scores'
}

const API_BASE = 'http://localhost:3001/api';
const WS_URL = 'ws://localhost:3001/ws';

// localStorage utilities
const STORAGE_KEY = 'trivia_player_name';

const getStoredPlayerName = (): string => {
  return localStorage.getItem(STORAGE_KEY) || '';
};

const setStoredPlayerName = (name: string): void => {
  localStorage.setItem(STORAGE_KEY, name);
};

const clearStoredPlayerName = (): void => {
  localStorage.removeItem(STORAGE_KEY);
};

function Home() {
  const navigate = useNavigate();
  const [playerName, setPlayerName] = useState('');
  const [showNamePrompt, setShowNamePrompt] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    const storedName = getStoredPlayerName();
    if (storedName) {
      setPlayerName(storedName);
    } else {
      setShowNamePrompt(true);
    }
  }, []);

  const saveName = () => {
    if (!playerName.trim()) return;
    setStoredPlayerName(playerName.trim());
    setShowNamePrompt(false);
  };

  const changeName = () => {
    setShowNamePrompt(true);
  };

  const clearName = () => {
    clearStoredPlayerName();
    setPlayerName('');
    setShowNamePrompt(true);
  };

  const createRoom = async () => {
    if (!playerName.trim()) {
      setShowNamePrompt(true);
      return;
    }
    
    setIsLoading(true);
    try {
      const response = await axios.post(`${API_BASE}/create-room`, {
        player_name: playerName.trim()
      });
      
      // Navigate to the generated room
      navigate(`/${response.data.room_name}`);
    } catch (error) {
      console.error('Failed to create room:', error);
      alert('Failed to create room. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const joinRoom = () => {
    if (!playerName.trim()) {
      setShowNamePrompt(true);
      return;
    }

    const roomName = prompt('Enter room name:');
    if (roomName && roomName.trim()) {
      navigate(`/${roomName.trim()}`);
    }
  };

  if (showNamePrompt) {
    return (
      <div className="screen name-prompt-screen">
        <h1>Welcome to Trivia!</h1>
        <p className="name-prompt-text">What's your name?</p>
        <div className="name-prompt-form">
          <input
            type="text"
            placeholder="Enter your name"
            value={playerName}
            onChange={(e) => setPlayerName(e.target.value)}
            onKeyPress={(e) => e.key === 'Enter' && saveName()}
            className="name-input"
            autoFocus
          />
          <button onClick={saveName} className="save-name-button">
            Continue
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="screen home-screen">
      <h1>Trivia</h1>
      <div className="player-greeting">
        <p>Welcome back, <strong>{playerName}</strong>!</p>
        <div className="name-actions">
          <button onClick={changeName} className="change-name-button">
            Change Name
          </button>
          <button onClick={clearName} className="clear-name-button">
            Clear Name
          </button>
        </div>
      </div>
      <div className="home-options">
        <button 
          onClick={createRoom} 
          className="option-button" 
          disabled={isLoading}
        >
          {isLoading ? 'Creating...' : 'Create Room'}
        </button>
        <button onClick={joinRoom} className="option-button">
          Join Room
        </button>
      </div>
    </div>
  );
}

function GameRoom() {
  const { roomName } = useParams<{ roomName: string }>();
  const navigate = useNavigate();
  
  const [gameState, setGameState] = useState<GameState>(GameState.JOIN_ROOM);
  const [playerName, setPlayerName] = useState('');
  const [currentPlayer, setCurrentPlayer] = useState<Player | null>(null);
  const [players, setPlayers] = useState<Player[]>([]);
  const [currentQuestion, setCurrentQuestion] = useState('');
  const [questionNumber, setQuestionNumber] = useState(0);
  const [fixedNumQuestions, setFixedNumQuestions] = useState(10);
  const [answer, setAnswer] = useState('');
  const [submittedAnswer, setSubmittedAnswer] = useState('');
  const [hasSubmitted, setHasSubmitted] = useState(false);
  const [isReadyForNext, setIsReadyForNext] = useState(false);
  const [results, setResults] = useState<PlayerResult[]>([]);
  const [finalScores, setFinalScores] = useState<Player[]>([]);
  const wsRef = useRef<WebSocket | null>(null);
  const [isHost, setIsHost] = useState(false);
  const [correctAnswer, setCorrectAnswer] = useState<string>('');

  const fetchAllPlayers = useCallback(async (currentRoomName: string | undefined) => {
    if (!currentRoomName) return;
    try {
      const response = await axios.get(`${API_BASE}/room/${currentRoomName}/players`);
      setPlayers(response.data);
    } catch (error) {
      console.error('Failed to fetch players:', error);
      // Optionally, handle the error, e.g., show a message to the user
    }
  }, [setPlayers]);

  const connectWebSocket = useCallback(() => {
    if (!roomName) return;

    if (wsRef.current) {
      if (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING) {
        wsRef.current.onclose = null; 
        wsRef.current.onerror = null;
        wsRef.current.onmessage = null;
        wsRef.current.onopen = null;
        wsRef.current.close();
      }
      wsRef.current = null;
    }
    
    const websocket = new WebSocket(`${WS_URL}?room=${encodeURIComponent(roomName)}`);
    wsRef.current = websocket;
    
    websocket.onopen = () => {
      console.log('WebSocket connected (ref)');
    };
    
    websocket.onmessage = (event) => {
      const message: GameMessage = JSON.parse(event.data);
      console.log('Received message (ref):', message);
      
      switch (message.type) {
        case 'PlayerJoined':
          if (roomName) {
            fetchAllPlayers(roomName);
          }
          break;
        case 'PlayerReadyStateChanged':
          if (message.player_id && typeof message.is_ready === 'boolean') {
            setPlayers(prevPlayers => 
              prevPlayers.map(p => 
                p.id === message.player_id ? { ...p, is_ready_to_start: message.is_ready! } : p
              )
            );
          }
          break;
        case 'GameStarted':
          setGameState(GameState.QUESTION);
          if (typeof message.num_questions === 'number' && message.num_questions > 0) {
            setFixedNumQuestions(message.num_questions);
          } else {
            setFixedNumQuestions(10);
          }
          break;
        case 'QuestionPresented':
          setCurrentQuestion(message.question || '');
          setQuestionNumber(message.question_number || 0);
          setAnswer('');
          setSubmittedAnswer('');
          setHasSubmitted(false);
          setIsReadyForNext(false);
          setGameState(GameState.QUESTION);
          break;
        case 'ResultsShown':
          setResults(message.results || []);
          setCorrectAnswer(message.correct_answer || '');
          setIsReadyForNext(false);
          setGameState(GameState.RESULTS);
          break;
        case 'GameEnded':
          setFinalScores(message.final_scores || []);
          setGameState(GameState.FINAL_SCORES);
          break;
      }
    };
    
    websocket.onclose = () => {
      console.log('WebSocket disconnected (ref)');
      if (wsRef.current === websocket) {
        wsRef.current = null;
        console.log('Attempting to reconnect WebSocket...');
        setTimeout(connectWebSocket, 3000);
      }
    };
    
    websocket.onerror = (error) => {
      console.error('WebSocket error (ref):', error);
    };
  }, [
    roomName,
    setPlayers, setCorrectAnswer,
    fetchAllPlayers, setFixedNumQuestions
  ]);

  const joinRoomWithStoredName = useCallback(async (name: string) => {
    if (!name.trim() || !roomName) return;
    
    try {
      const response = await axios.post(`${API_BASE}/join-room`, {
        room_name: roomName,
        player_name: name.trim()
      });
      
      setCurrentPlayer(response.data.player);
      setIsHost(response.data.is_host);
      await fetchAllPlayers(roomName);
      
      setGameState(GameState.WAITING);
      connectWebSocket(); // connectWebSocket is now stable
    } catch (error) {
      console.error('Failed to join room:', error);
      alert('Failed to join room. Room may not exist.');
      navigate('/'); 
    }
  }, [roomName, connectWebSocket, navigate, setCurrentPlayer, setIsHost, fetchAllPlayers, setGameState]);

  const joinRoomWithName = useCallback(async () => {
    if (!playerName.trim() || !roomName) return;
    setStoredPlayerName(playerName.trim());
    
    try {
      const response = await axios.post(`${API_BASE}/join-room`, {
        room_name: roomName,
        player_name: playerName.trim()
      });
      
      setCurrentPlayer(response.data.player);
      setIsHost(response.data.is_host);
      await fetchAllPlayers(roomName);
      
      setGameState(GameState.WAITING);
      connectWebSocket(); // connectWebSocket is now stable
    } catch (error) {
      console.error('Failed to join room:', error);
      alert('Failed to join room. Room may not exist.');
      navigate('/');
    }
  }, [playerName, roomName, connectWebSocket, navigate, setCurrentPlayer, setIsHost, fetchAllPlayers, setGameState]);

  // useEffect to auto-join room, which will then call connectWebSocket internally
  useEffect(() => {
    if (roomName && gameState === GameState.JOIN_ROOM && !currentPlayer) {
      const storedName = getStoredPlayerName();
      if (storedName) {
        setPlayerName(storedName); 
        joinRoomWithStoredName(storedName);
      } else {
        navigate('/'); // If no name, cannot join, redirect to home
      }
    }
  }, [roomName, gameState, currentPlayer, joinRoomWithStoredName, navigate, setPlayerName]);

  // useEffect for WebSocket cleanup on component unmount
  useEffect(() => {
    return () => {
      if (wsRef.current) {
        console.log('GameRoom unmounting, closing WebSocket (ref)');
        wsRef.current.onclose = null; // Prevent automatic reconnection attempts
        wsRef.current.onerror = null;
        wsRef.current.onmessage = null;
        wsRef.current.onopen = null;
        if (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING) {
          wsRef.current.close();
        }
        wsRef.current = null;
      }
    };
  }, []); // Empty dependency array ensures this runs only on mount and unmount (for cleanup)

  const renderJoinRoomScreen = () => (
    <div className="screen join-room-screen">
      <h1>Join Room: {roomName}</h1>
      <div className="room-form">
        <input
          type="text"
          placeholder="Enter your name"
          value={playerName}
          onChange={(e) => setPlayerName(e.target.value)}
          onKeyPress={(e) => e.key === 'Enter' && joinRoomWithName()}
          className="name-input"
        />
        <button onClick={joinRoomWithName} className="join-button">
          Join Room
        </button>
        <button onClick={() => navigate('/')} className="back-button">
          Back to Home
        </button>
      </div>
    </div>
  );

  const renderWaitingScreen = () => (
    <div className="screen waiting-screen">
      <h1>Trivia Lobby</h1>
      <h2>Players Joined:</h2>
      <div className="players-list">
        {players.map(player => (
          <div key={player.id} className={`player-item ${player.is_ready_to_start ? 'ready' : 'not-ready'}`}>
            {player.name}
            {player.id === currentPlayer?.id && ' (You)'}
            {isHost && player.id === currentPlayer?.id && <span className="host-tag"> - HOST</span>}
            <span className="ready-status">
              {player.is_ready_to_start ? '✅ Ready' : '⏳ Not Ready'}
            </span>
          </div>
        ))}
      </div>
      
      {currentPlayer && (
        <button 
          onClick={togglePlayerReadyToStart} 
          className={`action-button ${currentPlayer.is_ready_to_start ? 'unready-button' : 'ready-button'}`}
          disabled={gameState !== GameState.WAITING}
        >
          {currentPlayer.is_ready_to_start ? 'Mark as Not Ready' : 'Mark as Ready'}
        </button>
      )}
      
      <p className="waiting-message">
        <span className="waiting-icon">⏳</span>
        Game will start automatically when all players are ready.
        ({players.filter(p => p.is_ready_to_start).length}/{players.length} players ready)
      </p>
      {isHost && players.length < 1 && (
         <p className="waiting-message small-text">Minimum 1 player to start.</p>
      )}
    </div>
  );

  const renderQuestionScreen = () => (
    <div className="screen question-screen">
      <div className="question-header">
        <h2>Question {questionNumber} of {fixedNumQuestions}</h2>
      </div>
      <div className="question-content">
        <h3>{currentQuestion}</h3>
        {!hasSubmitted ? (
          <div className="answer-form">
            <input
              type="text"
              placeholder="Enter your answer"
              value={answer}
              onChange={(e) => setAnswer(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && submitAnswer()}
              className="answer-input"
            />
            <button onClick={submitAnswer} className="submit-button">
              Submit Answer
            </button>
          </div>
        ) : (
          <div className="submitted-answer">
            <div className="your-answer">
              <strong>Your answer:</strong> "{submittedAnswer}"
            </div>
            <div className="waiting-message">
              <span className="waiting-icon">⏳</span>
              {players.length > 1 
                ? "Waiting for other players to submit their answers..."
                : "Answer submitted! Processing results..."
              }
            </div>
          </div>
        )}
      </div>
    </div>
  );

  const renderResultsScreen = () => {
    return (
      <div className="screen results-screen">
        <h2>Results for Question {questionNumber}</h2>
        
        <div className="correct-answer-display">
          <h3>✅ Correct Answer: <span className="correct-answer-text">{correctAnswer}</span></h3>
        </div>
        
        <div className="results-list">
          {results.map((result, index) => {
            const getResultClass = () => {
              switch (result.correctness) {
                case 'Correct': return 'correct';
                case 'Partial': return 'partial';
                case 'Wrong': return 'incorrect';
                default: return 'incorrect';
              }
            };
            
            const getResultIcon = () => {
              switch (result.correctness) {
                case 'Correct': return '✅ Correct (+2 points)';
                case 'Partial': return '🟡 Partially Correct (+1 point)';
                case 'Wrong': return '❌ Incorrect (0 points)';
                default: return '❌ Incorrect (0 points)';
              }
            };
            
            return (
              <div key={index} className={`result-item ${getResultClass()}`}>
                <div className="result-content">
                  <span className="player-name">{result.player_name}</span>
                  <span className="player-answer">"{result.answer}"</span>
                  <span className="result-indicator">
                    {getResultIcon()}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
        
        {!isReadyForNext ? (
          <button onClick={readyForNext} className="next-button">
            Ready for Next Question
          </button>
        ) : (
          <div className="waiting-message">
            <span className="waiting-icon">⏳</span>
            Waiting for other players to be ready...
          </div>
        )}
      </div>
    );
  };

  const renderFinalScoresScreen = () => {
    const sortedScores = [...finalScores].sort((a, b) => b.score - a.score);
    const currentPlayerDetails = sortedScores.find(p => p.id === currentPlayer?.id);
    const currentPlayerRank = currentPlayerDetails ? sortedScores.indexOf(currentPlayerDetails) + 1 : -1;
    
    return (
      <div className="screen final-scores-screen">
        {currentPlayerRank > 0 && currentPlayerRank <= 3 && currentPlayerDetails ? (
          <div className="victory-section">
            <div className="victory-card-3d">
              <div className={`victory-card-face victory-card-front rank-${currentPlayerRank}`}>
                <div className="victory-crown">
                  {currentPlayerRank === 1 && '👑'}
                  {currentPlayerRank === 2 && '🥈'}
                  {currentPlayerRank === 3 && '🥉'}
                </div>
                
                <div className="victory-rank-display">
                  <div className={`victory-rank-number rank-${currentPlayerRank}`}>
                    {currentPlayerRank === 1 ? '1ST' : currentPlayerRank === 2 ? '2ND' : '3RD'}
                  </div>
                  <div className="victory-title-3d">
                    {currentPlayerRank === 1 && 'CHAMPION'}
                    {currentPlayerRank === 2 && 'RUNNER-UP'}
                    {currentPlayerRank === 3 && 'THIRD PLACE'}
                  </div>
                </div>
                
                <div className="victory-player-name">
                  {currentPlayerDetails.name}
                </div>
                
                <div className="victory-score-display">
                  <div className="victory-score-label">Final Score</div>
                  <div className={`victory-score-value rank-${currentPlayerRank}`}>
                    {currentPlayerDetails.score}/{fixedNumQuestions * 2}
                  </div>
                </div>
                
                <button 
                  onClick={() => downloadBadge(currentPlayerRank, currentPlayerDetails.name, currentPlayerDetails.score)}
                  className="victory-download-btn"
                >
                  🏅 Download Badge
                </button>
              </div>
            </div>
          </div>
        ) : (
          <div className="participation-section">
            <h1>🎉 Thanks for Playing! 🎉</h1>
            <p className="participation-message">
              {currentPlayerRank > 0 
                ? `You finished in ${currentPlayerRank}${getOrdinalSuffix(currentPlayerRank)} place.` 
                : 'Great effort!'}
            </p>
          </div>
        )}
        
        <div className="final-leaderboard">
          <h2>🏆 Final Leaderboard</h2>
          <div className="scores-list">
            {sortedScores.map((player, index) => (
              <div key={player.id} className={`score-item ${index === 0 ? 'winner' : ''} ${index < 3 ? 'podium' : ''}`}>
                <span className="rank">
                  {index === 0 && '🥇'}
                  {index === 1 && '🥈'}
                  {index === 2 && '🥉'}
                  {index > 2 && `#${index + 1}`}
                </span>
                <span className="player-name">{player.name}{player.id === currentPlayer?.id && " (You)"}</span>
                <span className="score">{player.score} / {fixedNumQuestions * 2}</span>
                {index < 3 && (
                  <button 
                    onClick={() => downloadBadge(index + 1, player.name, player.score)}
                    className="mini-download-button"
                    title="Download badge"
                  >
                    🏅
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>
        
        <button onClick={() => window.location.reload()} className="play-again-button">
          🔄 Play Again
        </button>
      </div>
    );
  };

  const getOrdinalSuffix = (n: number) => {
    const s = ["th", "st", "nd", "rd"];
    const v = n % 100;
    return s[(v - 20) % 10] || s[v] || s[0];
  };

  const submitAnswer = async () => {
    if (!answer.trim() || !currentPlayer || !roomName) return;
    
    try {
      await axios.post(`${API_BASE}/submit-answer`, {
        player_id: currentPlayer.id,
        answer: answer.trim(),
        room_name: roomName
      });
      
      setSubmittedAnswer(answer.trim());
      setHasSubmitted(true);
      setAnswer('');
    } catch (error) {
      console.error('Failed to submit answer:', error);
      alert('Failed to submit answer. Please try again.');
    }
  };

  const readyForNext = async () => {
    if (!currentPlayer || !roomName) return;
    
    try {
      await axios.post(`${API_BASE}/ready-next`, {
        player_id: currentPlayer.id,
        room_name: roomName
      });
      
      setIsReadyForNext(true);
    } catch (error) {
      console.error('Failed to ready for next:', error);
      alert('Failed to ready for next question. Please try again.');
    }
  };

  const togglePlayerReadyToStart = async () => {
    if (!currentPlayer || !roomName) return;
    try {
      await axios.post(`${API_BASE}/player-ready-to-start`, {
        player_id: currentPlayer.id,
        room_name: roomName
      });
    } catch (error) {
      console.error('Failed to toggle ready state:', error);
      alert('Failed to toggle ready state. Please try again.');
    }
  };

  const generateVictoryBadge = (rank: number, playerName: string, score: number) => {
    const colors = {
      1: { primary: '#FFD700', secondary: '#FFA500', accent: '#FF6B35' },
      2: { primary: '#C0C0C0', secondary: '#A9A9A9', accent: '#4169E1' },
      3: { primary: '#CD7F32', secondary: '#D2691E', accent: '#8B4513' }
    };
    
    const color = colors[rank as keyof typeof colors] || colors[3];
    const titles = { 1: 'CHAMPION', 2: 'RUNNER-UP', 3: 'THIRD PLACE' };
    const title = titles[rank as keyof typeof titles] || 'PARTICIPANT';
    
    return `
      <svg width="400" height="500" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="bgGradient${rank}" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" style="stop-color:${color.primary};stop-opacity:1" />
            <stop offset="100%" style="stop-color:${color.secondary};stop-opacity:1" />
          </linearGradient>
          <filter id="shadow${rank}">
            <feDropShadow dx="3" dy="3" stdDeviation="3" flood-opacity="0.3"/>
          </filter>
          <pattern id="stars${rank}" patternUnits="userSpaceOnUse" width="50" height="50">
            <circle cx="25" cy="25" r="2" fill="${color.accent}" opacity="0.3"/>
            <circle cx="10" cy="10" r="1" fill="${color.accent}" opacity="0.2"/>
            <circle cx="40" cy="15" r="1.5" fill="${color.accent}" opacity="0.25"/>
          </pattern>
        </defs>
        
        <!-- Background -->
        <rect width="400" height="500" fill="url(#bgGradient${rank})"/>
        <rect width="400" height="500" fill="url(#stars${rank})"/>
        
        <!-- Border -->
        <rect x="10" y="10" width="380" height="480" fill="none" stroke="${color.accent}" stroke-width="4" rx="20"/>
        
        <!-- Crown/Medal -->
        ${rank === 1 ? `
          <g transform="translate(200, 80)">
            <polygon points="-30,-20 -15,-40 0,-35 15,-40 30,-20 20,-10 10,-15 0,-10 -10,-15 -20,-10" 
                     fill="#FFD700" stroke="#FFA500" stroke-width="2" filter="url(#shadow${rank})"/>
            <circle cx="-15" cy="-25" r="4" fill="#FF6B35"/>
            <circle cx="0" cy="-30" r="5" fill="#FF6B35"/>
            <circle cx="15" cy="-25" r="4" fill="#FF6B35"/>
          </g>
        ` : `
          <circle cx="200" cy="70" r="35" fill="${color.primary}" stroke="${color.accent}" stroke-width="3" filter="url(#shadow${rank})"/>
          <text x="200" y="80" text-anchor="middle" font-family="Arial, sans-serif" font-size="24" font-weight="bold" fill="${color.accent}">${rank}</text>
        `}
        
        <!-- Title -->
        <text x="200" y="150" text-anchor="middle" font-family="Arial, sans-serif" font-size="28" font-weight="bold" fill="white" filter="url(#shadow${rank})">${title}</text>
        
        <!-- Player Name -->
        <text x="200" y="200" text-anchor="middle" font-family="Arial, sans-serif" font-size="24" font-weight="bold" fill="white">${playerName}</text>
        
        <!-- Score -->
        <g transform="translate(200, 280)">
          <rect x="-80" y="-30" width="160" height="60" fill="white" fill-opacity="0.9" rx="10" filter="url(#shadow${rank})"/>
          <text x="0" y="-5" text-anchor="middle" font-family="Arial, sans-serif" font-size="16" fill="${color.accent}">FINAL SCORE</text>
          <text x="0" y="20" text-anchor="middle" font-family="Arial, sans-serif" font-size="32" font-weight="bold" fill="${color.accent}">${score}/${fixedNumQuestions * 2}</text>
        </g>
        
        <!-- Trivia Game Logo -->
        <text x="200" y="380" text-anchor="middle" font-family="Arial, sans-serif" font-size="20" font-weight="bold" fill="white" opacity="0.8">🧠 TRIVIA CHAMPION 🧠</text>
        
        <!-- Date -->
        <text x="200" y="420" text-anchor="middle" font-family="Arial, sans-serif" font-size="14" fill="white" opacity="0.7">${new Date().toLocaleDateString()}</text>
        
        <!-- Decorative elements -->
        <g opacity="0.3">
          <circle cx="50" cy="100" r="3" fill="white"/>
          <circle cx="350" cy="150" r="2" fill="white"/>
          <circle cx="80" cy="400" r="2.5" fill="white"/>
          <circle cx="320" cy="380" r="2" fill="white"/>
          <circle cx="60" cy="300" r="1.5" fill="white"/>
          <circle cx="340" cy="280" r="2" fill="white"/>
        </g>
      </svg>
    `;
  };

  const downloadBadge = (rank: number, playerName: string, score: number) => {
    const svgContent = generateVictoryBadge(rank, playerName, score);
    const blob = new Blob([svgContent], { type: 'image/svg+xml' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `trivia-badge-${playerName.replace(/\s+/g, '-').toLowerCase()}-place-${rank}.svg`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  };

  // Check if roomName exists, redirect to home if not
  if (!roomName) {
    navigate('/');
    return null;
  }

  return (
    <div className="App game-room-container">
      <button onClick={() => navigate('/')} className="back-to-home-button">
        🏠 Back to Home
      </button>
      {gameState === GameState.JOIN_ROOM && renderJoinRoomScreen()}
      {gameState === GameState.WAITING && renderWaitingScreen()}
      {gameState === GameState.QUESTION && renderQuestionScreen()}
      {gameState === GameState.RESULTS && renderResultsScreen()}
      {gameState === GameState.FINAL_SCORES && renderFinalScoresScreen()}
    </div>
  );
}

function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/:roomName" element={<GameRoom />} />
      </Routes>
    </Router>
  );
}

export default App;