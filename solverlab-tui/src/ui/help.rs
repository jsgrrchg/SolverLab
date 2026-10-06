//! Key bindings popup (`?`).

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

const BINDINGS: [(&str, &str); 15] = [
    ("Tab / Shift+Tab", "Move focus"),
    ("Enter / Space", "Activate focused control"),
    ("← →", "Change game (on Game) / move focus"),
    ("g / G", "Next / previous game"),
    ("0-9, Backspace", "Edit Sims / Parallel"),
    ("↑ ↓ (Sims, Parallel)", "±1 (Shift: ±10)"),
    ("a", "Toggle auto parallel"),
    ("s", "Start / Stop"),
    ("e", "Export CSV"),
    ("c", "Clear data"),
    ("↑ ↓ PgUp PgDn Home End", "Scroll results"),
    ("[ ]", "Sort column (always descending)"),
    ("?", "Toggle this help"),
    ("q / Esc / Ctrl+C", "Quit"),
    ("", "Config is locked while a run is active"),
];

pub fn render(frame: &mut Frame, area: Rect) {
    let popup = area.centered(
        Constraint::Length(64),
        Constraint::Length(BINDINGS.len() as u16 + 2),
    );
    let lines: Vec<Line> = BINDINGS
        .iter()
        .map(|(keys, description)| {
            Line::from(vec![
                Span::styled(
                    format!(" {keys:<24}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(*description),
            ])
        })
        .collect();
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Keys ")),
        popup,
    );
}
