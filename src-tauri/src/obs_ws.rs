use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tauri;

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
    /// Update desired state. The actor connects, disconnects, or reconnects
    /// with fresh credentials to match — no app restart needed.
    Configure { enabled: bool, password: String },
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

/// Spawns the OBS WebSocket actor. Always spawned at startup regardless of
/// config — while disabled it idles waiting for a `Configure`; while enabled
/// it reconnects forever with capped backoff, so OBS starting *after* this
/// app (or restarting mid-session) is picked up automatically.
pub fn spawn_obs_ws(
    enabled: bool,
    password: String,
) -> (mpsc::Sender<ObsWsCommand>, mpsc::Receiver<ObsWsEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<ObsWsCommand>(16);
    let (event_tx, event_rx) = mpsc::channel::<ObsWsEvent>(64);

    tauri::async_runtime::spawn(actor(cmd_rx, event_tx, enabled, password));

    (cmd_tx, event_rx)
}

/// Delay before the next connection attempt. A session that actually
/// connected resets `attempt` to 0, so a drop mid-session retries fast;
/// repeated failures back off to a 30s cap (OBS simply not running).
fn reconnect_delay(attempt: u32) -> std::time::Duration {
    let secs = match attempt {
        0 => 3,
        1 => 5,
        2 => 10,
        3 => 20,
        _ => 30,
    };
    std::time::Duration::from_secs(secs)
}

async fn actor(
    mut cmd_rx: mpsc::Receiver<ObsWsCommand>,
    event_tx: mpsc::Sender<ObsWsEvent>,
    mut enabled: bool,
    mut password: String,
) {
    let mut attempt: u32 = 0;

    loop {
        if !enabled {
            let _ = event_tx
                .send(ObsWsEvent::StatusChanged {
                    status: "disabled".to_string(),
                    attempt: None,
                })
                .await;
            match cmd_rx.recv().await {
                Some(ObsWsCommand::Configure { enabled: e, password: p }) => {
                    enabled = e;
                    password = p;
                    attempt = 0;
                }
                None => return,
            }
            continue;
        }

        let _ = event_tx
            .send(ObsWsEvent::StatusChanged {
                status: "connecting".to_string(),
                attempt: Some(attempt + 1),
            })
            .await;

        // Run the connection, but stay responsive to Configure — dropping
        // the connect_and_run future tears the socket down.
        let mut reconfigured = false;
        tokio::select! {
            res = connect_and_run(&password, event_tx.clone()) => {
                match res {
                    Ok(was_connected) => attempt = if was_connected { 0 } else { attempt + 1 },
                    Err(_) => attempt += 1,
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(ObsWsCommand::Configure { enabled: e, password: p }) => {
                        enabled = e;
                        password = p;
                        attempt = 0;
                        reconfigured = true;
                    }
                    None => return,
                }
            }
        }
        if reconfigured {
            continue;
        }

        // Backoff before the next attempt, still responsive to Configure.
        tokio::select! {
            _ = tokio::time::sleep(reconnect_delay(attempt)) => {}
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(ObsWsCommand::Configure { enabled: e, password: p }) => {
                        enabled = e;
                        password = p;
                        attempt = 0;
                    }
                    None => return,
                }
            }
        }
    }
}

/// Returns `Ok(true)` if the session reached the Identified state before
/// ending (used to reset reconnect backoff), `Ok(false)` for a clean close
/// before identifying, `Err` for connection/protocol failures.
async fn connect_and_run(
    password: &str,
    event_tx: mpsc::Sender<ObsWsEvent>,
) -> Result<bool, String> {
    let url = "ws://localhost:4455";
    let (ws_stream, _response) = connect_async(url)
        .await
        .map_err(|e| format!("connect failed: {e}"))?;

    let (mut write, mut read) = ws_stream.split();
    let mut identified = false;

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
                        identified = true;
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
                return Ok(identified);
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
    Ok(identified)
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
