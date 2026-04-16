use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone)]
pub enum ObsWsEvent {
    Connected,
    Disconnected { code: Option<u16>, reason: String },
    ReplayBufferSaved { path: String },
    AuthError { message: String },
    Error { message: String },
    StatusChanged { status: String, attempt: Option<u32> },
}

#[derive(Debug)]
pub enum ObsWsCommand {
    Connect,
    Disconnect,
}

pub fn build_auth_string(password: &str, salt: &str, challenge: &str) -> String {
    // SHA256(password + salt) -> base64 -> secret
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    let secret_bytes = hasher.finalize();
    let secret = STANDARD.encode(secret_bytes);

    // SHA256(secret + challenge) -> base64 -> auth string
    let mut hasher2 = Sha256::new();
    hasher2.update(secret.as_bytes());
    hasher2.update(challenge.as_bytes());
    let auth_bytes = hasher2.finalize();
    STANDARD.encode(auth_bytes)
}

pub fn spawn_obs_ws(
    password: String,
    max_retries: u32,
) -> (mpsc::Sender<ObsWsCommand>, mpsc::Receiver<ObsWsEvent>) {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<ObsWsCommand>(16);
    let (event_tx, event_rx) = mpsc::channel::<ObsWsEvent>(64);

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                ObsWsCommand::Connect => {
                    let mut retry_count = 0u32;
                    loop {
                        let _ = event_tx
                            .send(ObsWsEvent::StatusChanged {
                                status: "connecting".to_string(),
                                attempt: Some(retry_count + 1),
                            })
                            .await;

                        match connect_and_run(&password, event_tx.clone()).await {
                            Ok(_) => {
                                // Clean disconnect — stop retrying
                                break;
                            }
                            Err(e) => {
                                retry_count += 1;
                                let _ = event_tx
                                    .send(ObsWsEvent::StatusChanged {
                                        status: format!("retry-{retry_count}: {e}"),
                                        attempt: Some(retry_count),
                                    })
                                    .await;

                                if retry_count >= max_retries {
                                    let _ = event_tx
                                        .send(ObsWsEvent::StatusChanged {
                                            status: "failed-over".to_string(),
                                            attempt: Some(retry_count),
                                        })
                                        .await;
                                    break;
                                }

                                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            }
                        }
                    }
                }
                ObsWsCommand::Disconnect => {
                    // Nothing to do here if not connected; a running
                    // connect_and_run will handle its own teardown.
                }
            }
        }
    });

    (cmd_tx, event_rx)
}

async fn connect_and_run(
    password: &str,
    event_tx: mpsc::Sender<ObsWsEvent>,
) -> Result<(), String> {
    let url = "ws://localhost:4455";
    let (ws_stream, _response) = connect_async(url)
        .await
        .map_err(|e| format!("connect failed: {e}"))?;

    let (mut write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                let _ = event_tx
                    .send(ObsWsEvent::Disconnected {
                        code: None,
                        reason: format!("read error: {e}"),
                    })
                    .await;
                return Err(format!("read error: {e}"));
            }
        };

        match msg {
            Message::Text(text) => {
                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = event_tx
                            .send(ObsWsEvent::Error {
                                message: format!("JSON parse error: {e}"),
                            })
                            .await;
                        continue;
                    }
                };

                let op = match value.get("op").and_then(|v| v.as_u64()) {
                    Some(v) => v,
                    None => continue,
                };

                match op {
                    // op=0: Hello
                    0 => {
                        let d = value.get("d");
                        let auth_field = d
                            .and_then(|d| d.get("authentication"));

                        let payload = if let Some(auth) = auth_field {
                            let salt = auth
                                .get("salt")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let challenge = auth
                                .get("challenge")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let auth_string = build_auth_string(password, salt, challenge);
                            serde_json::json!({
                                "op": 1,
                                "d": {
                                    "rpcVersion": 1,
                                    "authentication": auth_string,
                                    "eventSubscriptions": 1000
                                }
                            })
                        } else {
                            serde_json::json!({
                                "op": 1,
                                "d": {
                                    "rpcVersion": 1,
                                    "eventSubscriptions": 1000
                                }
                            })
                        };

                        let msg_text = serde_json::to_string(&payload)
                            .map_err(|e| format!("serialize error: {e}"))?;
                        write
                            .send(Message::Text(msg_text.into()))
                            .await
                            .map_err(|e| format!("send error: {e}"))?;
                    }

                    // op=2: Identified
                    2 => {
                        let _ = event_tx
                            .send(ObsWsEvent::StatusChanged {
                                status: "connected".to_string(),
                                attempt: None,
                            })
                            .await;
                        let _ = event_tx.send(ObsWsEvent::Connected).await;
                    }

                    // op=5: Event
                    5 => {
                        let d = match value.get("d") {
                            Some(d) => d,
                            None => continue,
                        };
                        let event_type = d
                            .get("eventType")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if event_type == "ReplayBufferSaved" {
                            let raw_path = d
                                .get("eventData")
                                .and_then(|ed| ed.get("savedReplayPath"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            // Convert forward slashes to backslashes (Windows paths)
                            let path = raw_path.replace('/', "\\");
                            let _ = event_tx
                                .send(ObsWsEvent::ReplayBufferSaved { path })
                                .await;
                        }
                    }

                    _ => {}
                }
            }

            Message::Close(frame) => {
                let (code, reason) = match frame {
                    Some(f) => (Some(u16::from(f.code)), f.reason.to_string()),
                    None => (None, "connection closed".to_string()),
                };
                let _ = event_tx
                    .send(ObsWsEvent::Disconnected { code, reason })
                    .await;
                return Ok(());
            }

            // Ignore ping/pong/binary
            _ => {}
        }
    }

    // Stream ended without explicit close frame
    let _ = event_tx
        .send(ObsWsEvent::Disconnected {
            code: None,
            reason: "stream ended".to_string(),
        })
        .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_auth_string_deterministic() {
        let password = "testpassword";
        let salt = "testsalt";
        let challenge = "testchallenge";

        let result1 = build_auth_string(password, salt, challenge);
        let result2 = build_auth_string(password, salt, challenge);

        assert_eq!(result1, result2, "same inputs should produce same output");
        assert!(!result1.is_empty(), "auth string should not be empty");
    }

    #[test]
    fn test_build_auth_string_different_inputs() {
        let salt = "somesalt";
        let challenge = "somechallenge";

        let result_a = build_auth_string("password_alpha", salt, challenge);
        let result_b = build_auth_string("password_beta", salt, challenge);

        assert_ne!(
            result_a, result_b,
            "different passwords should produce different auth strings"
        );
    }
}
