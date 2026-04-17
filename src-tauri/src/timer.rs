use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

use crate::events::TimerTickPayload;

pub enum TimerCommand {
    Start { duration_secs: u32 },
    Stop,
    Reset { duration_secs: u32 },
}

/// Spawn a timer task that listens for commands and emits tick events.
/// Returns a sender for sending commands to the timer.
pub fn spawn_timer(app_handle: AppHandle) -> mpsc::Sender<TimerCommand> {
    let (tx, mut rx) = mpsc::channel::<TimerCommand>(32);

    tauri::async_runtime::spawn(async move {
        let mut total_secs: u32 = 70;
        let mut remaining: u32 = 0;
        let mut running = false;
        let mut tick_interval = interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(TimerCommand::Start { duration_secs }) => {
                            total_secs = duration_secs;
                            remaining = duration_secs;
                            running = true;
                        }
                        Some(TimerCommand::Stop) => {
                            running = false;
                        }
                        Some(TimerCommand::Reset { duration_secs }) => {
                            total_secs = duration_secs;
                            remaining = duration_secs;
                            running = false;
                            let _ = app_handle.emit("timer-tick", TimerTickPayload {
                                remaining_secs: remaining,
                                total_secs,
                            });
                        }
                        None => break, // Channel closed
                    }
                }
                _ = tick_interval.tick(), if running => {
                    if remaining > 0 {
                        remaining -= 1;
                        let _ = app_handle.emit("timer-tick", TimerTickPayload {
                            remaining_secs: remaining,
                            total_secs,
                        });
                    }
                    if remaining == 0 && running {
                        running = false;
                        let _ = app_handle.emit("timer-expired", ());
                    }
                }
            }
        }
    });

    tx
}
