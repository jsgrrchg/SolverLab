// Modules are wired into the UI incrementally; allow unused items until then.
#[allow(dead_code)]
mod csv;
#[allow(dead_code)]
mod deck;
#[allow(dead_code)]
mod format;
#[allow(dead_code)]
mod model;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::{Block, Paragraph};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| -> color_eyre::Result<()> {
        loop {
            terminal.draw(|frame| {
                let body = Paragraph::new("SolverLab TUI — press q to quit")
                    .block(Block::bordered().title(" SolverLab "));
                frame.render_widget(body, frame.area());
            })?;
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
            {
                return Ok(());
            }
        }
    })
}
