// Modules are wired into the UI incrementally; allow unused items until then.
#[allow(dead_code)]
mod app;
#[allow(dead_code)]
mod csv;
#[allow(dead_code)]
mod deck;
#[allow(dead_code)]
mod format;
#[allow(dead_code)]
mod games;
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod persist;
#[allow(dead_code)]
mod runtime;
#[allow(dead_code)]
mod ui;

use std::sync::mpsc;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use app::App;
use deck::ShuffleSource;
use persist::ConfigStore;
use ui::ViewState;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let store = ConfigStore::default_path().map_or_else(ConfigStore::disabled, ConfigStore::at);
    let (tx, _rx) = mpsc::channel();
    let app = App::new(store.load(), ShuffleSource::Random, tx).with_store(store);
    let mut view = ViewState::new(false);

    ratatui::run(|terminal| -> color_eyre::Result<()> {
        loop {
            terminal.draw(|frame| ui::draw(frame, &app, &mut view))?;
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
            {
                return Ok(());
            }
        }
    })
}
