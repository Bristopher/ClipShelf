use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::events::TimerTickPayload;

#[derive(Debug)]
pub enum CountUpCommand {
    /// Single-key toggle: if running, reset to 0 and stop; if stopped,
    /// start counting up from 0.
    Toggle,
    /// Force-stop and zero the stopwatch regardless of current state
    /// (overlay "reset" control — distinct from Toggle, which only resets
    /// when it was already running).
    Reset,
}

pub enum TimerCommand {
    Start { duration_secs: u32 },
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

/// Spawn a count-up timer task. Single command — `Toggle` — flips between
/// running (counting up by 1s) and reset-and-stopped (at 0). Emits
/// `tick_event` every second with the elapsed seconds (0 when stopped).
pub fn spawn_count_up_timer(
    app_handle: AppHandle,
    tick_event: &'static str,
) -> mpsc::Sender<CountUpCommand> {
    let (tx, mut rx) = mpsc::channel::<CountUpCommand>(32);

    tauri::async_runtime::spawn(async move {
        let mut elapsed: u32 = 0;
        let mut running = false;
        let mut tick_interval = interval(Duration::from_secs(1));
        // Same reasoning as the countdown timer: prevent backlogged ticks
        // from draining all at once when running flips true.
        tick_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(CountUpCommand::Toggle) => {
                            if running {
                                running = false;
                                elapsed = 0;
                            } else {
                                elapsed = 0;
                                running = true;
                            }
                            let _ = app_handle.emit(tick_event, CountUpTickPayload {
                                elapsed_secs: elapsed,
                                running,
                            });
                        }
                        Some(CountUpCommand::Reset) => {
                            running = false;
                            elapsed = 0;
                            let _ = app_handle.emit(tick_event, CountUpTickPayload {
                                elapsed_secs: elapsed,
                                running,
                            });
                        }
                        None => break,
                    }
                }
                _ = tick_interval.tick(), if running => {
                    elapsed = elapsed.saturating_add(1);
                    let _ = app_handle.emit(tick_event, CountUpTickPayload {
                        elapsed_secs: elapsed,
                        running,
                    });
                }
            }
        }
    });

    tx
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CountUpTickPayload {
    pub elapsed_secs: u32,
    pub running: bool,
}
