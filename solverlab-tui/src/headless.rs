//! Headless mode: same runtime and CSV as the TUI, without a terminal UI.

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Instant;

use crate::app::App;
use crate::persist::write_atomic;
use crate::runtime::RunEvent;

pub fn run(
    app: &mut App,
    rx: &Receiver<RunEvent>,
    csv_path: Option<&Path>,
    quiet: bool,
) -> io::Result<()> {
    let tty = io::stderr().is_terminal();
    let total = app.config.simulations.max(0);
    app.start();

    let mut last_tick = Instant::now();
    while app.is_running {
        match rx.recv_timeout(crate::TICK) {
            Ok(RunEvent::GameFinished { run_id, result }) => {
                if !quiet && !tty {
                    eprintln!(
                        "game {}: {} {} moves={} score={} {:.2}s",
                        result.id,
                        result.result_label(),
                        result.stop_reason.raw_value(),
                        result.move_count,
                        result.score,
                        result.duration
                    );
                }
                app.handle_event(RunEvent::GameFinished { run_id, result });
            }
            Ok(event) => app.handle_event(event),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if last_tick.elapsed() >= crate::TICK {
            app.tick(Instant::now());
            last_tick = Instant::now();
            if !quiet && tty {
                eprint!(
                    "\r{}/{} games · win rate {:.2}% · ETA {}   ",
                    app.completed_games,
                    total,
                    app.win_rate() * 100.0,
                    app.estimated_remaining_display()
                );
            }
        }
    }
    if !quiet && tty {
        eprintln!(
            "\r{}/{} games · win rate {:.2}%            ",
            app.completed_games,
            total,
            app.win_rate() * 100.0
        );
    }

    let csv = app.csv();
    match csv_path {
        Some(path) => write_atomic(path, csv.as_bytes()),
        None => io::stdout().lock().write_all(csv.as_bytes()),
    }
}
