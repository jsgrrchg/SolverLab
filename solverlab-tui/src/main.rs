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
