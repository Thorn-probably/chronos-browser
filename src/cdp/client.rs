use crate::error::{ChronosError, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, trace, warn};

/// An outgoing CDP JSON-RPC request.
#[derive(Debug, Serialize)]
pub struct CdpRequest {
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// An incoming CDP event notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpEvent {
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(rename = "sessionId", default)]
    pub session_id: Option<String>,
}

/// An asynchronous client for Chrome DevTools Protocol over WebSockets.
pub struct CdpClient {
    next_id: AtomicU64,
    outgoing_tx: mpsc::UnboundedSender<Message>,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value>>>>>,
    event_tx: broadcast::Sender<CdpEvent>,
    reader_handle: tokio::task::JoinHandle<()>,
    writer_handle: tokio::task::JoinHandle<()>,
}

impl CdpClient {
    /// Connects to a Chrome DevTools WebSocket URL and spawns background reader/writer tasks.
    pub async fn connect(ws_url: &str) -> Result<Arc<Self>> {
        debug!("Connecting to CDP at {}", ws_url);
        let (ws_stream, _) = connect_async(ws_url).await.map_err(|e| {
            ChronosError::ConnectionFailed(format!("Failed to connect to {}: {}", ws_url, e))
        })?;

        let (mut ws_writer, mut ws_reader) = ws_stream.split();
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<Message>();
        let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel::<CdpEvent>(2048);

        // Writer task: forwards messages from channel to WebSocket sink
        let writer_handle = tokio::spawn(async move {
            while let Some(msg) = outgoing_rx.recv().await {
                if let Err(e) = ws_writer.send(msg).await {
                    error!("CDP WebSocket write error: {}", e);
                    break;
                }
            }
        });

        // Reader task: parses incoming frames and routes responses/events
        let reader_pending = Arc::clone(&pending_requests);
        let reader_events = event_tx.clone();
        let reader_handle = tokio::spawn(async move {
            while let Some(msg_res) = ws_reader.next().await {
                match msg_res {
                    Ok(Message::Text(text)) => {
                        trace!("CDP incoming: {}", text);
                        let val: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(e) => {
                                warn!("Failed to parse CDP message JSON: {}", e);
                                continue;
                            }
                        };

                        // Check if it's a response to a request (has "id")
                        if let Some(id) = val.get("id").and_then(|i| i.as_u64()) {
                            let mut pending = reader_pending.lock().await;
                            if let Some(tx) = pending.remove(&id) {
                                if let Some(err_val) = val.get("error") {
                                    let code =
                                        err_val.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                                    let message = err_val
                                        .get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown CDP error")
                                        .to_string();
                                    let data = err_val.get("data").map(|d| d.to_string());
                                    let _ = tx.send(Err(ChronosError::ProtocolError {
                                        code,
                                        message,
                                        data,
                                    }));
                                } else {
                                    let result = val
                                        .get("result")
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Null);
                                    let _ = tx.send(Ok(result));
                                }
                            }
                        } else if let Some(method) = val.get("method").and_then(|m| m.as_str()) {
                            // Event notification
                            let params =
                                val.get("params").cloned().unwrap_or(serde_json::Value::Null);
                            let session_id = val
                                .get("sessionId")
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string());

                            let event = CdpEvent {
                                method: method.to_string(),
                                params,
                                session_id,
                            };
                            let _ = reader_events.send(event);
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        debug!("CDP WebSocket connection closed: {:?}", frame);
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        debug!("CDP WebSocket connection terminated: {}", e);
                        break;
                    }
                }
            }

            // Reader terminated; fail all in-flight pending requests
            let mut pending = reader_pending.lock().await;
            for (_, tx) in pending.drain() {
                let _ = tx.send(Err(ChronosError::ConnectionFailed(
                    "CDP WebSocket connection was closed".to_string(),
                )));
            }
        });

        Ok(Arc::new(Self {
            next_id: AtomicU64::new(1),
            outgoing_tx,
            pending_requests,
            event_tx,
            reader_handle,
            writer_handle,
        }))
    }

    /// Sends a raw CDP command and awaits its correlated response.
    pub async fn send_command(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
        session_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        let req = CdpRequest {
            id,
            method: method.to_string(),
            params,
            session_id: session_id.map(|s| s.to_string()),
        };

        let json_str = serde_json::to_string(&req)?;
        self.outgoing_tx
            .send(Message::Text(json_str))
            .map_err(|e| ChronosError::ChannelClosed(e.to_string()))?;

        // 30-second default command timeout
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ChronosError::ConnectionFailed(
                "Response channel closed before receiving result".to_string(),
            )),
            Err(_) => {
                // Remove from pending map on timeout
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                Err(ChronosError::Timeout(format!(
                    "CDP request {} ('{}') timed out after 30s",
                    id, method
                )))
            }
        }
    }

    /// Sends a command to the browser-level target (without sessionId).
    pub async fn send_browser(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.send_command(method, params, None).await
    }

    /// Sends a command to a specific session target (e.g. an attached page).
    pub async fn send_session(
        &self,
        session_id: &str,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.send_command(method, params, Some(session_id)).await
    }

    /// Subscribes to all CDP events emitted across all sessions and the browser.
    pub fn subscribe_events(&self) -> broadcast::Receiver<CdpEvent> {
        self.event_tx.subscribe()
    }
}

impl Drop for CdpClient {
    fn drop(&mut self) {
        self.reader_handle.abort();
        self.writer_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdp_request_serialization() {
        let req = CdpRequest {
            id: 1,
            method: "Page.navigate".to_string(),
            params: Some(serde_json::json!({ "url": "https://example.com" })),
            session_id: Some("session-123".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"Page.navigate\""));
        assert!(json.contains("\"sessionId\":\"session-123\""));
        assert!(json.contains("\"url\":\"https://example.com\""));
    }

    #[test]
    fn test_cdp_event_deserialization() {
        let json = r#"{"method":"Page.loadEventFired","params":{"timestamp":12345.67},"sessionId":"s1"}"#;
        let event: CdpEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.method, "Page.loadEventFired");
        assert_eq!(event.session_id.as_deref(), Some("s1"));
        assert_eq!(event.params["timestamp"], 12345.67);
    }
}

