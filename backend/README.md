# Backend - Trivia Game API

## Environment Variables

Before running the backend, you need to set up your environment variables.

1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```

2. Edit the `.env` file and add your actual API tokens:
   ```
   SHOPIFY_API_TOKEN=your_actual_shopify_api_token_here
   ```

## Running the Backend

```bash
cargo run
```

The server will start on `http://localhost:3001`.

## Environment Variables Required

- `SHOPIFY_API_TOKEN`: Your Shopify AI API token for generating trivia questions and checking answers

## API Endpoints

- `POST /api/create-room` - Create a new trivia room
- `POST /api/join-room` - Join an existing room
- `POST /api/player-ready-to-start` - Mark player as ready to start
- `POST /api/submit-answer` - Submit an answer to a question
- `POST /api/ready-next` - Mark player as ready for next question
- `GET /api/generate-trivia` - Generate trivia questions
- `GET /ws` - WebSocket connection for real-time updates
- `GET /api/room/:room_name/players` - Get players in a room 