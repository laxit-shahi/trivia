import React, { useState, useEffect, useCallback } from 'react';
import axios from 'axios';
import './App.css';

interface Player {
  id: string;
  name: string;
  score: number;
  current_answer?: string;
  ready_for_next: boolean;
}

interface PlayerResult {
  player_name: string;
  answer: string;
  correct: boolean;
}

interface GameMessage {
  type: string;
  player?: Player;
  question?: string;
  question_number?: number;
  player_id?: string;
  answer?: string;
  results?: PlayerResult[];
  final_scores?: Player[];
  message?: string;
}

enum GameState {
  JOIN = 'join',
  WAITING = 'waiting',
  QUESTION = 'question',
  RESULTS = 'results',
  FINAL_SCORES = 'final_scores'
}

const API_BASE = 'http://localhost:3001/api';
const WS_URL = 'ws://localhost:3001/ws';

function App() {
  const [gameState, setGameState] = useState<GameState>(GameState.JOIN);
  const [playerName, setPlayerName] = useState('');
  const [currentPlayer, setCurrentPlayer] = useState<Player | null>(null);
  const [players, setPlayers] = useState<Player[]>([]);
  const [currentQuestion, setCurrentQuestion] = useState('');
  const [questionNumber, setQuestionNumber] = useState(0);
  const [answer, setAnswer] = useState('');
  const [submittedAnswer, setSubmittedAnswer] = useState('');
  const [hasSubmitted, setHasSubmitted] = useState(false);
  const [isReadyForNext, setIsReadyForNext] = useState(false);
  const [results, setResults] = useState<PlayerResult[]>([]);
  const [finalScores, setFinalScores] = useState<Player[]>([]);
  const [ws, setWs] = useState<WebSocket | null>(null);
  const [isHost, setIsHost] = useState(false);

  const connectWebSocket = useCallback(() => {
    const websocket = new WebSocket(WS_URL);
    
    websocket.onopen = () => {
      console.log('WebSocket connected');
      setWs(websocket);
    };
    
    websocket.onmessage = (event) => {
      const message: GameMessage = JSON.parse(event.data);
      console.log('Received message:', message);
      
      switch (message.type) {
        case 'PlayerJoined':
          if (message.player) {
            setPlayers(prev => {
              const existing = prev.find(p => p.id === message.player!.id);
              if (existing) return prev;
              return [...prev, message.player!];
            });
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
      console.log('WebSocket disconnected');
      setWs(null);
      // Attempt to reconnect after 3 seconds
      setTimeout(connectWebSocket, 3000);
    };
    
    websocket.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
  }, []);

  useEffect(() => {
    connectWebSocket();
    
    return () => {
      if (ws) {
        ws.close();
      }
    };
  }, [connectWebSocket]);

  const joinGame = async () => {
    if (!playerName.trim()) return;
    
    try {
      const response = await axios.post(`${API_BASE}/join`, {
        name: playerName.trim()
      });
      
      setCurrentPlayer(response.data.player);
      setIsHost(response.data.is_host);
      setGameState(GameState.WAITING);
    } catch (error) {
      console.error('Failed to join game:', error);
      alert('Failed to join game. Please try again.');
    }
  };

  const startGame = async () => {
    try {
      await axios.post(`${API_BASE}/start`);
      // const ok = await axios.get(`${API_BASE}/generate_trivia_questions`);
      // console.log(ok);
    } catch (error) {
      console.error('Failed to start game:', error);
      alert('Failed to start game. Please try again.');
    }
  };

  const submitAnswer = async () => {
    if (!currentPlayer || !answer.trim() || hasSubmitted) return;
    
    const trimmedAnswer = answer.trim();
    
    try {
      await axios.post(`${API_BASE}/submit-answer`, {
        player_id: currentPlayer.id,
        answer: trimmedAnswer
      });
      
      setSubmittedAnswer(trimmedAnswer);
      setHasSubmitted(true);
      setAnswer('');
    } catch (error) {
      console.error('Failed to submit answer:', error);
      alert('Failed to submit answer. Please try again.');
    }
  };

  const readyForNext = async () => {
    if (!currentPlayer || isReadyForNext) return;
    
    try {
      await axios.post(`${API_BASE}/ready-next`, {
        player_id: currentPlayer.id
      });
      
      setIsReadyForNext(true);
    } catch (error) {
      console.error('Failed to mark ready:', error);
      alert('Failed to mark ready. Please try again.');
    }
  };

  const renderJoinScreen = () => (
    <div className="screen join-screen">
      <h1>Trivia</h1>
      <div className="join-form">
        <input
          type="text"
          placeholder="Enter your name"
          value={playerName}
          onChange={(e) => setPlayerName(e.target.value)}
          onKeyPress={(e) => e.key === 'Enter' && joinGame()}
          className="name-input"
        />
        <button onClick={joinGame} className="join-button">
          Join Game
        </button>
      </div>
    </div>
  );

  const renderWaitingScreen = () => (
    <div className="screen waiting-screen">
      <h1>Trivia</h1>
      <h2>Players Joined:</h2>
      <div className="players-list">
        {players.map(player => (
          <div key={player.id} className="player-item">
            {player.name}
            {player.id === currentPlayer?.id && ' (You)'}
          </div>
        ))}
      </div>
      {isHost && (
        <button onClick={startGame} className="start-button">
          Start Game
        </button>
      )}
      {!isHost && (
        <p className="waiting-message">Waiting for host to start the game...</p>
      )}
    </div>
  );

  const renderQuestionScreen = () => (
    <div className="screen question-screen">
      <div className="question-header">
        <h2>Question {questionNumber} of 10</h2>
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

  const renderResultsScreen = () => (
    <div className="screen results-screen">
      <h2>Results for Question {questionNumber}</h2>
      <div className="results-list">
        {results.map((result, index) => (
          <div key={index} className={`result-item ${result.correct ? 'correct' : 'incorrect'}`}>
            <span className="player-name">{result.player_name}</span>
            <span className="player-answer">"{result.answer}"</span>
            <span className="result-indicator">
              {result.correct ? '✅ Correct' : '❌ Incorrect'}
            </span>
          </div>
        ))}
      </div>
      {!isReadyForNext ? (
        <button onClick={readyForNext} className="next-button">
          Ready for Next Question
        </button>
      ) : (
        <div className="waiting-message">
          <span className="waiting-icon">⏳</span>
          {players.length > 1 
            ? "Waiting for other players to be ready..."
            : "Loading next question..."
          }
        </div>
      )}
    </div>
  );

  const renderFinalScoresScreen = () => {
    const sortedScores = [...finalScores].sort((a, b) => b.score - a.score);
    
    return (
      <div className="screen final-scores-screen">
        <h1>🏆 Final Scores</h1>
        <div className="scores-list">
          {sortedScores.map((player, index) => (
            <div key={player.id} className={`score-item ${index === 0 ? 'winner' : ''}`}>
              <span className="rank">#{index + 1}</span>
              <span className="player-name">{player.name}</span>
              <span className="score">{player.score}/10</span>
              {index === 0 && <span className="crown">👑</span>}
            </div>
          ))}
        </div>
        <button onClick={() => window.location.reload()} className="play-again-button">
          Play Again
        </button>
      </div>
    );
  };

  return (
    <div className="App">
      {gameState === GameState.JOIN && renderJoinScreen()}
      {gameState === GameState.WAITING && renderWaitingScreen()}
      {gameState === GameState.QUESTION && renderQuestionScreen()}
      {gameState === GameState.RESULTS && renderResultsScreen()}
      {gameState === GameState.FINAL_SCORES && renderFinalScoresScreen()}
    </div>
  );
}

export default App;