use axum::{
    extract::{ws::WebSocket, ws::WebSocketUpgrade, State},
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::collections::HashMap;
use crate::types::SharedAppState;

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<SharedAppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Response {
    let room_name = params.get("room").cloned().unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, app_state, room_name))
}

pub async fn handle_socket(socket: WebSocket, app_state: SharedAppState, room_name: String) {
    let game_data_opt = {
        let app = app_state.lock().unwrap();
        app.rooms.get(&room_name).cloned()
    };
    
    if let Some(game_data) = game_data_opt {
        let (mut sender, mut receiver) = socket.split();
        let mut rx = {
            let game = game_data.lock().unwrap();
            game.tx.subscribe()
        };
        
        let mut send_task = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if sender
                    .send(axum::extract::ws::Message::Text(
                        serde_json::to_string(&msg).unwrap(),
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        
        let mut recv_task = tokio::spawn(async move {
            while let Some(Ok(_)) = receiver.next().await {
                // We don't need to handle incoming WebSocket messages for this app
                // All communication goes through HTTP endpoints
            }
        });
        
        tokio::select! {
            _ = (&mut send_task) => {
                recv_task.abort();
            },
            _ = (&mut recv_task) => {
                send_task.abort();
            },
        }
    }
} 