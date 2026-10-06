//! Progress bar with `completed/total (pct%)`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Gauge};

use crate::app::App;

pub const HEIGHT: u16 = 3;

pub fn label(app: &App) -> String {
    let total = app.config.simulations.max(1);
    let percent = (app.progress_fraction() * 100.0).round() as i64;
    format!("{}/{} ({}%)", app.completed_games, total, percent)
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let gauge = Gauge::default()
        .block(Block::bordered().title(" Progress "))
        .gauge_style(Style::default().fg(Color::Cyan))
        .use_unicode(true)
        .ratio(app.progress_fraction().clamp(0.0, 1.0))
        .label(label(app));
    frame.render_widget(gauge, area);
}
