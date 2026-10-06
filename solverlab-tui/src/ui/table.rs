//! Results table. Like the Swift `Table`, sorting is always descending.

use std::cmp::Ordering;
use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::{Constraint, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table};

use super::{Focus, SortColumn, ViewState};
use crate::app::App;
use crate::model::SimResult;

const WIDTHS: [u16; 8] = [9, 7, 7, 12, 8, 12, 9, 7];

fn compare(a: &SimResult, b: &SimResult, column: SortColumn) -> Ordering {
    match column {
        SortColumn::GameId => a.id.cmp(&b.id),
        SortColumn::Moves => a.move_count.cmp(&b.move_count),
        SortColumn::Undos => a.undo_count.cmp(&b.undo_count),
        SortColumn::Checkpoints => a.checkpoint_sort_value().cmp(&b.checkpoint_sort_value()),
        SortColumn::Result => a.result_label().cmp(b.result_label()),
        SortColumn::StopReason => a.stop_reason.label().cmp(b.stop_reason.label()),
        SortColumn::Duration => a.duration.total_cmp(&b.duration),
        SortColumn::Score => a.score.cmp(&b.score),
    }
}

/// Rows sorted descending by `column`, ties broken by descending game id.
pub fn sorted_rows(results: &VecDeque<SimResult>, column: SortColumn) -> Vec<&SimResult> {
    let mut rows: Vec<&SimResult> = results.iter().collect();
    rows.sort_by(|a, b| compare(b, a, column).then_with(|| b.id.cmp(&a.id)));
    rows
}

pub fn render(frame: &mut Frame, area: Rect, app: &App, view: &mut ViewState) {
    let glyphs = view.glyphs;
    let rows = sorted_rows(&app.results, view.sort_column);

    let header = Row::new(SortColumn::ALL.iter().map(|&column| {
        let title = if column == view.sort_column {
            format!("{} {}", column.title(), glyphs.descending)
        } else {
            column.title().to_string()
        };
        Cell::from(title)
    }))
    .style(Style::default().add_modifier(Modifier::BOLD));

    let body = rows.iter().map(|row| {
        let (mark, color) = if row.won {
            (glyphs.won, Color::Green)
        } else {
            (glyphs.lost, Color::Red)
        };
        Row::new([
            Cell::from(row.id.to_string()),
            Cell::from(row.move_count.to_string()),
            Cell::from(row.undo_count.to_string()),
            Cell::from(row.checkpoint_count_label()),
            Cell::from(Line::styled(
                format!("{mark} {}", row.result_label()),
                Style::default().fg(color),
            )),
            Cell::from(row.stop_reason.label()),
            Cell::from(format!("{:.2}s", row.duration)),
            Cell::from(row.score.to_string()),
        ])
    });

    let focused = view.focus == Focus::Table;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let block = Block::bordered()
        .border_style(border_style)
        .title(format!(
            " Results (sorted by {} {}) ",
            view.sort_column.title(),
            glyphs.descending
        ))
        .title(Line::from(format!(" {} rows ", rows.len())).right_aligned());

    let highlight = if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let table = Table::new(body, WIDTHS.map(Constraint::Length))
        .header(header)
        .block(block)
        .row_highlight_style(highlight);

    if view.table.selected().is_none() && !rows.is_empty() {
        view.table.select(Some(0));
    }
    if let Some(selected) = view.table.selected()
        && selected >= rows.len()
    {
        view.table.select(rows.len().checked_sub(1));
    }
    frame.render_stateful_widget(table, area, &mut view.table);

    let mut scrollbar_state =
        ScrollbarState::new(rows.len()).position(view.table.selected().unwrap_or(0));
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StopReason;

    fn row(id: u32, moves: i64, checkpoints: Option<i64>, duration: f64, won: bool) -> SimResult {
        SimResult {
            id,
            move_count: moves,
            undo_count: 0,
            checkpoint_count: checkpoints,
            won,
            stop_reason: if won {
                StopReason::Win
            } else {
                StopReason::Stalled
            },
            duration,
            score: moves * 2,
            moves_detail: String::new(),
        }
    }

    fn ids(results: &VecDeque<SimResult>, column: SortColumn) -> Vec<u32> {
        sorted_rows(results, column).iter().map(|r| r.id).collect()
    }

    #[test]
    fn sorting_is_always_descending() {
        let results: VecDeque<SimResult> = [
            row(1, 30, Some(2), 0.5, true),
            row(2, 10, None, 2.5, false),
            row(3, 20, Some(5), 1.5, true),
        ]
        .into_iter()
        .collect();

        assert_eq!(ids(&results, SortColumn::GameId), [3, 2, 1]);
        assert_eq!(ids(&results, SortColumn::Moves), [1, 3, 2]);
        assert_eq!(ids(&results, SortColumn::Score), [1, 3, 2]);
        assert_eq!(ids(&results, SortColumn::Checkpoints), [3, 1, 2]);
        assert_eq!(ids(&results, SortColumn::Duration), [2, 3, 1]);
        // "won" > "lost"; ties fall back to descending id.
        assert_eq!(ids(&results, SortColumn::Result), [3, 1, 2]);
        assert_eq!(ids(&results, SortColumn::Undos), [3, 2, 1]);
    }
}
