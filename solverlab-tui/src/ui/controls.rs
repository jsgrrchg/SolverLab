//! Top bar: game picker, numeric inputs, auto toggle, actions and status.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::{Focus, ViewState, focus_style};
use crate::app::App;

pub fn height(wide: bool) -> u16 {
    if wide { 3 } else { 4 }
}

/// Whether "Export CSV" is enabled (Swift: disabled when there are no results).
pub fn can_export(app: &App) -> bool {
    !app.results.is_empty()
}

/// Whether "Clear Data" is enabled.
pub fn can_clear(app: &App) -> bool {
    !app.results.is_empty() || app.completed_games > 0
}

fn numeric_field<'a>(
    label: &'a str,
    value: i64,
    focus: Focus,
    view: &ViewState,
    enabled: bool,
) -> Vec<Span<'a>> {
    let focused = view.focus == focus;
    let text = match (&view.editing, focused) {
        (Some(buffer), true) => format!("[{buffer}_]"),
        _ => format!("[{value}]"),
    };
    vec![
        Span::raw(format!("{label}: ")),
        Span::styled(text, focus_style(focused, enabled)),
        Span::raw("  "),
    ]
}

fn button<'a>(
    label: String,
    focus: Focus,
    view: &ViewState,
    enabled: bool,
    base: Style,
) -> Span<'a> {
    Span::styled(
        format!(" {label} "),
        base.patch(focus_style(view.focus == focus, enabled)),
    )
}

pub fn render(frame: &mut Frame, area: Rect, app: &App, view: &ViewState, wide: bool) {
    let glyphs = view.glyphs;
    let config = &app.config;
    let editable = !app.is_running;

    let mut settings = vec![Span::raw("Game: ")];
    settings.push(Span::styled(
        format!(
            "{}{}{}",
            glyphs.picker_left,
            config.game_type.ui_label(),
            glyphs.picker_right
        ),
        focus_style(view.focus == Focus::Game, editable),
    ));
    settings.push(Span::raw("  "));
    settings.extend(numeric_field(
        "Sims",
        config.simulations,
        Focus::Sims,
        view,
        editable,
    ));
    settings.extend(numeric_field(
        "Parallel",
        config.parallel_games,
        Focus::Parallel,
        view,
        editable && !config.auto_parallel,
    ));
    settings.push(Span::raw("Auto: "));
    settings.push(Span::styled(
        if config.auto_parallel { "[x]" } else { "[ ]" },
        focus_style(view.focus == Focus::Auto, editable),
    ));

    let (start_label, start_style) = if app.is_running {
        (
            format!("{} Stop", glyphs.stop),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            format!("{} Start", glyphs.play),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    };
    let actions = vec![
        button(start_label, Focus::StartStop, view, true, start_style),
        Span::raw(" "),
        button(
            "Export CSV".into(),
            Focus::Export,
            view,
            can_export(app),
            Style::default(),
        ),
        Span::raw(" "),
        button(
            "Clear Data".into(),
            Focus::Clear,
            view,
            can_clear(app),
            Style::default(),
        ),
    ];

    let lines = if wide {
        let mut line = settings;
        line.push(Span::raw(format!("  {}  ", glyphs.separator)));
        line.extend(actions);
        vec![Line::from(line)]
    } else {
        vec![Line::from(settings), Line::from(actions)]
    };

    let status_color = if app.is_running {
        Color::Green
    } else {
        Color::Gray
    };
    let status = Line::from(vec![
        Span::styled(
            format!(" {} ", glyphs.dot),
            Style::default().fg(status_color),
        ),
        Span::raw(format!("{} ", app.status_text)),
    ])
    .right_aligned();

    let block = Block::bordered().title(" SolverLab ").title(status);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
