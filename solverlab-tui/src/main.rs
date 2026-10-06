mod app;
mod csv;
// Seeds and config paths are wired up by the CLI.
#[allow(dead_code)]
mod deck;
mod format;
mod games;
mod input;
mod model;
#[allow(dead_code)]
mod persist;
mod runtime;
mod ui;

use std::panic;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

use app::App;
use deck::ShuffleSource;
use input::Flow;
use persist::ConfigStore;
use runtime::{GAME_THREAD_PREFIX, RunEvent};
use ui::ViewState;

/// Countdown/ETA refresh period (Swift: `runCountdown` sleeps 200 ms).
const TICK: Duration = Duration::from_millis(200);
const MAX_POLL: Duration = Duration::from_millis(100);

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let store = ConfigStore::default_path().map_or_else(ConfigStore::disabled, ConfigStore::at);
    let (tx, rx) = mpsc::channel();
    let mut app = App::new(store.load(), ShuffleSource::Random, tx).with_store(store);
    let mut view = ViewState::new(false);

    ratatui::run(|terminal| {
        silence_game_thread_panics();
        run(terminal, &mut app, &mut view, &rx)
    })
}

/// A panicking game is recorded as stalled by the runtime; keep its panic
/// from restoring the terminal (ratatui's hook) while the UI is still running.
fn silence_game_thread_panics() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let in_game_thread = thread::current()
            .name()
            .is_some_and(|name| name.starts_with(GAME_THREAD_PREFIX));
        if !in_game_thread {
            previous(info);
        }
    }));
}

fn run(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    view: &mut ViewState,
    rx: &mpsc::Receiver<RunEvent>,
) -> color_eyre::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|frame| ui::draw(frame, app, view))?;

        let timeout = TICK.saturating_sub(last_tick.elapsed()).min(MAX_POLL);
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && let Some(action) = input::map_key(key, view)
            && input::apply(action, app, view) == Flow::Quit
        {
            return Ok(());
        }

        while let Ok(event) = rx.try_recv() {
            app.handle_event(event);
        }
        if last_tick.elapsed() >= TICK {
            app.tick(Instant::now());
            last_tick = Instant::now();
        }
    }
}
