//! The three metric groups (status, results, averages), like the Swift `MetricBox` rows.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::App;

struct Metric {
    title: &'static str,
    value: String,
    tint: Color,
}

fn metric(title: &'static str, value: String, tint: Color) -> Metric {
    Metric { title, value, tint }
}

fn groups(app: &App) -> [(&'static str, Vec<Metric>); 3] {
    [
        (
            "Status",
            vec![
                metric("Games", app.completed_games.to_string(), Color::Blue),
                metric(
                    "Parallel used",
                    app.current_parallel_used.to_string(),
                    Color::Blue,
                ),
                metric("ETA", app.estimated_remaining_display(), Color::Blue),
                metric("Active timeout", app.countdown_display(), Color::Blue),
            ],
        ),
        (
            "Results",
            vec![
                metric(
                    "Win rate",
                    format!("{:.2}%", app.win_rate() * 100.0),
                    Color::Green,
                ),
                metric("Wins", app.win_count.to_string(), Color::Green),
                metric("Stalled", app.stalled_count.to_string(), Color::Yellow),
                metric("Timeout", app.timeout_count.to_string(), Color::Yellow),
            ],
        ),
        (
            "Averages",
            vec![
                metric(
                    "Avg moves",
                    format!("{:.2}", app.average_moves()),
                    Color::Magenta,
                ),
                metric(
                    "Avg undos",
                    format!("{:.2}", app.average_undos()),
                    Color::Magenta,
                ),
                metric(
                    "Avg duration",
                    format!("{:.2}s", app.average_duration()),
                    Color::Magenta,
                ),
            ],
        ),
    ]
}

pub fn height(wide: bool) -> u16 {
    if wide { 4 } else { 9 }
}

fn group_block(title: &str, tint: Color) -> Block<'_> {
    Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::default().fg(tint))
}

/// Cell width in the wide layout: the title plus some breathing room.
fn cell_width(metric: &Metric) -> u16 {
    metric.title.len().max(8) as u16 + 2
}

/// Wide: title above value in its own cell. Narrow: `title value` pairs on one line.
pub fn render(frame: &mut Frame, area: Rect, app: &App, wide: bool) {
    let groups = groups(app);
    if wide {
        let areas = Layout::horizontal(
            groups
                .iter()
                .map(|(_, metrics)| {
                    Constraint::Length(metrics.iter().map(cell_width).sum::<u16>() + 2)
                })
                .chain([Constraint::Fill(1)]),
        )
        .split(area);
        for ((title, metrics), group_area) in groups.iter().zip(areas.iter()) {
            let block = group_block(title, metrics[0].tint);
            let inner = block.inner(*group_area);
            frame.render_widget(block, *group_area);
            let cells =
                Layout::horizontal(metrics.iter().map(|m| Constraint::Length(cell_width(m))))
                    .split(inner);
            for (metric, cell) in metrics.iter().zip(cells.iter()) {
                let text = vec![
                    Line::styled(
                        metric.title,
                        Style::default().fg(metric.tint).add_modifier(Modifier::DIM),
                    ),
                    Line::styled(
                        metric.value.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ];
                frame.render_widget(Paragraph::new(text), *cell);
            }
        }
    } else {
        let areas = Layout::vertical([Constraint::Length(3); 3]).split(area);
        for ((title, metrics), group_area) in groups.iter().zip(areas.iter()) {
            let mut spans = Vec::new();
            for metric in metrics {
                spans.push(Span::styled(
                    format!("{} ", metric.title),
                    Style::default().fg(metric.tint),
                ));
                spans.push(Span::styled(
                    format!("{}   ", metric.value),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            }
            let paragraph =
                Paragraph::new(Line::from(spans)).block(group_block(title, metrics[0].tint));
            frame.render_widget(paragraph, *group_area);
        }
    }
}
