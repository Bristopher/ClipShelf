use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::events::TimerTickPayload;

pub enum TimerCommand {
    Start { duration_secs: u32 },
    Stop,
    Reset { duration_secs: u32 },
}

/// Spawn a timer task that listens for commands and emits tick events
/// under the given event names. Returns a sender for commands.
///
/// Two instances are spawned in `lib.rs`: one drives auto-wipe on file
/// arrival (events `timer-tick` / `timer-expired`), and a second drives
/// the manual Start button (`user-timer-tick` / `user-timer-expired`)
/// so users can run a prep countdown independently of the auto-wipe.
pub fn spawn_timer(
    app_handle: AppHandle,
    tick_event: &'static str,
    expired_event: &'static str,
) -> mpsc::Sender<TimerCommand> {
    let (tx, mut rx) = mpsc::channel::<TimerCommand>(32);

    tauri::async_runtime::spawn(async move {
        let mut total_secs: u32 = 70;
        let mut remaining: u32 = 0;
        let mut running = false;
        let mut tick_interval = interval(Duration::from_secs(1));
        // Without this, the interval's default `Burst` behavior would fire
        // every "missed" tick back-to-back the moment `running` flips true
        // — meaning if the task sat idle for 70+ seconds before the first
        // clip arrived, the countdown would drain in a single event-loop
        // iteration and `timer-expired` would fire immediately.
        tick_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(TimerCommand::Start { duration_secs }) => {
                            total_secs = duration_secs;
                            remaining = duration_secs;
                            running = true;
                            let _ = app_handle.emit(tick_event, TimerTickPayload {
                                remaining_secs: remaining,
                                total_secs,
                            });
                        }
                        Some(TimerCommand::Stop) => {
                            running = false;
                        }
                        Some(TimerCommand::Reset { duration_secs }) => {
                            total_secs = duration_secs;
                            remaining = duration_secs;
                            running = false;
                            let _ = app_handle.emit(tick_event, TimerTickPayload {
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
                        let _ = app_handle.emit(tick_event, TimerTickPayload {
                            remaining_secs: remaining,
                            total_secs,
                        });
                    }
                    if remaining == 0 && running {
                        running = false;
                        let _ = app_handle.emit(expired_event, ());
                    }
                }
            }
        }
    });

    tx
}
