//! Rendering, ported from `ContentView.swift`. `draw` is a pure function of
//! the app state plus view-only state (focus, sorting, scroll).

mod controls;
mod metrics;
mod progress;
mod table;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::TableState;

use crate::app::App;

/// Terminal width from which metrics and controls fit on a single row.
const WIDE_LAYOUT_MIN_WIDTH: u16 = 130;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Game,
    Sims,
    Parallel,
    Auto,
    StartStop,
    Export,
    Clear,
    Table,
}

impl Focus {
    pub const ALL: [Focus; 8] = [
        Focus::Game,
        Focus::Sims,
        Focus::Parallel,
        Focus::Auto,
        Focus::StartStop,
        Focus::Export,
        Focus::Clear,
        Focus::Table,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|&f| f == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortColumn {
    #[default]
    GameId,
    Moves,
    Undos,
    Checkpoints,
    Result,
    StopReason,
    Duration,
    Score,
}

impl SortColumn {
    pub const ALL: [SortColumn; 8] = [
        SortColumn::GameId,
        SortColumn::Moves,
        SortColumn::Undos,
        SortColumn::Checkpoints,
        SortColumn::Result,
        SortColumn::StopReason,
        SortColumn::Duration,
        SortColumn::Score,
    ];

    pub fn title(self) -> &'static str {
        match self {
            SortColumn::GameId => "Game ID",
            SortColumn::Moves => "Moves",
            SortColumn::Undos => "Undos",
            SortColumn::Checkpoints => "Checkpoints",
            SortColumn::Result => "Result",
            SortColumn::StopReason => "Stop Reason",
            SortColumn::Duration => "Duration",
            SortColumn::Score => "Score",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&c| c == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Unicode glyphs with an ASCII fallback for limited consoles (`--ascii`).
#[derive(Clone, Copy, Debug)]
pub struct Glyphs {
    pub won: &'static str,
    pub lost: &'static str,
    pub play: &'static str,
    pub stop: &'static str,
    pub dot: &'static str,
    pub picker_left: &'static str,
    pub picker_right: &'static str,
    pub descending: &'static str,
    pub separator: &'static str,
}

impl Glyphs {
    pub const UNICODE: Glyphs = Glyphs {
        won: "✓",
        lost: "✗",
        play: "▶",
        stop: "■",
        dot: "●",
        picker_left: "‹",
        picker_right: "›",
        descending: "↓",
        separator: "│",
    };

    pub const ASCII: Glyphs = Glyphs {
        won: "+",
        lost: "x",
        play: ">",
        stop: "#",
        dot: "*",
        picker_left: "<",
        picker_right: ">",
        descending: "v",
        separator: "|",
    };
}

pub struct ViewState {
    pub focus: Focus,
    /// Text buffer while a numeric field is being edited.
    pub editing: Option<String>,
    pub sort_column: SortColumn,
    pub table: TableState,
    pub show_help: bool,
    pub glyphs: Glyphs,
}

impl ViewState {
    pub fn new(ascii: bool) -> Self {
        ViewState {
            focus: Focus::StartStop,
            editing: None,
            sort_column: SortColumn::default(),
            table: TableState::default(),
            show_help: false,
            glyphs: if ascii {
                Glyphs::ASCII
            } else {
                Glyphs::UNICODE
            },
        }
    }
}

pub(crate) fn focus_style(focused: bool, enabled: bool) -> Style {
    let mut style = Style::default();
    if !enabled {
        style = style.add_modifier(Modifier::DIM);
    }
    if focused {
        style = style.add_modifier(Modifier::REVERSED);
    }
    style
}

pub fn draw(frame: &mut Frame, app: &App, view: &mut ViewState) {
    let area = frame.area();
    let wide = area.width >= WIDE_LAYOUT_MIN_WIDTH;
    let [
        controls_area,
        progress_area,
        metrics_area,
        table_area,
        footer_area,
    ] = Layout::vertical([
        Constraint::Length(controls::height(wide)),
        Constraint::Length(progress::HEIGHT),
        Constraint::Length(metrics::height(wide)),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    controls::render(frame, controls_area, app, view, wide);
    progress::render(frame, progress_area, app);
    metrics::render(frame, metrics_area, app, wide);
    table::render(frame, table_area, app, view);
    frame.render_widget(footer(), footer_area);
}

fn footer() -> Line<'static> {
    Line::from(" Tab focus · s start/stop · e export · c clear · [ ] sort · ? help · q quit")
        .style(Style::default().add_modifier(Modifier::DIM))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::tests::{idle_app, result};
    use crate::model::{GameType, SimConfig, StopReason};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    pub(crate) fn render(app: &App, view: &mut ViewState, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app, view)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    pub(crate) fn sample_app() -> App {
        let (mut app, _rx) = idle_app(SimConfig {
            game_type: GameType::FreeCell,
            simulations: 3,
            parallel_games: 2,
            ..SimConfig::default()
        });
        app.record(result(1, true, StopReason::Win));
        app.record(result(2, false, StopReason::Stalled));
        app.record(result(3, true, StopReason::Win));
        app
    }

    #[test]
    fn wide_layout_shows_all_sections() {
        let app = sample_app();
        let mut view = ViewState::new(false);
        let screen = render(&app, &mut view, 150, 40);
        for needle in [
            "SolverLab",
            "Running 3/3",
            "FreeCell",
            "Progress",
            "3/3 (100%)",
            "Win rate",
            "66.67%",
            "Avg duration",
            "1.50s",
            "Game ID",
            "Stop Reason",
            "✓ won",
            "✗ lost",
            "Start",
            "Export CSV",
            "Clear Data",
        ] {
            assert!(screen.contains(needle), "missing {needle:?} in:\n{screen}");
        }
    }

    #[test]
    fn narrow_layout_renders_without_panicking() {
        let app = sample_app();
        let mut view = ViewState::new(true);
        let screen = render(&app, &mut view, 80, 24);
        assert!(screen.contains("Win rate"));
        assert!(screen.contains("+ won"));
        // Tiny terminals must not panic either.
        render(&app, &mut view, 20, 5);
    }

    #[test]
    #[ignore = "prints the rendered screens for manual inspection"]
    fn print_screens() {
        let app = sample_app();
        let mut view = ViewState::new(false);
        println!("{}", render(&app, &mut view, 150, 20));
        println!("{}", render(&app, &mut view, 80, 24));
    }

    #[test]
    fn focus_and_sort_cycle() {
        assert_eq!(Focus::Table.next(), Focus::Game);
        assert_eq!(Focus::Game.prev(), Focus::Table);
        assert_eq!(SortColumn::Score.next(), SortColumn::GameId);
        assert_eq!(SortColumn::GameId.prev(), SortColumn::Score);
    }
}
