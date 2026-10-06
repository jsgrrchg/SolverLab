//! Keyboard handling: `map_key` turns key presses into actions (pure) and
//! `apply` runs them against the app and view state.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use std::path::PathBuf;

use crate::app::App;
use crate::ui::controls::can_clear;
use crate::ui::dialogs::ExportDialog;
use crate::ui::{Focus, ViewState};

const MAX_DIGITS: usize = 9;
const PAGE_ROWS: u16 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    FocusNext,
    FocusPrev,
    Activate,
    GameNext,
    GamePrev,
    ToggleAuto,
    StartStop,
    Clear,
    Export,
    EditChar(char),
    EditBackspace,
    EditCommit,
    EditCancel,
    Increment(i64),
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    Top,
    Bottom,
    SortNext,
    SortPrev,
    ToggleHelp,
    /// Key routed to the open export dialog.
    Dialog(KeyCode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

fn is_numeric(focus: Focus) -> bool {
    matches!(focus, Focus::Sims | Focus::Parallel)
}

pub fn map_key(key: KeyEvent, view: &ViewState) -> Option<Action> {
    // Windows reports releases too; only presses count.
    if key.kind != KeyEventKind::Press {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }
    if view.export.is_some() {
        return Some(Action::Dialog(key.code));
    }
    if view.show_help {
        return matches!(
            key.code,
            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter
        )
        .then_some(Action::ToggleHelp);
    }

    let numeric = is_numeric(view.focus);
    if view.editing.is_some() {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_digit() => return Some(Action::EditChar(c)),
            KeyCode::Backspace => return Some(Action::EditBackspace),
            KeyCode::Enter => return Some(Action::EditCommit),
            KeyCode::Esc => return Some(Action::EditCancel),
            _ => {}
        }
    }

    let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
        10
    } else {
        1
    };
    Some(match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Enter | KeyCode::Char(' ') => Action::Activate,
        KeyCode::Esc | KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('s') => Action::StartStop,
        KeyCode::Char('c') => Action::Clear,
        KeyCode::Char('e') => Action::Export,
        KeyCode::Char('a') => Action::ToggleAuto,
        KeyCode::Char('g') => Action::GameNext,
        KeyCode::Char('G') => Action::GamePrev,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('[') => Action::SortPrev,
        KeyCode::Char(']') => Action::SortNext,
        KeyCode::Char(c) if numeric && c.is_ascii_digit() => Action::EditChar(c),
        KeyCode::Backspace if numeric => Action::EditBackspace,
        KeyCode::Left if view.focus == Focus::Game => Action::GamePrev,
        KeyCode::Right if view.focus == Focus::Game => Action::GameNext,
        KeyCode::Left => Action::FocusPrev,
        KeyCode::Right => Action::FocusNext,
        KeyCode::Up if numeric => Action::Increment(step),
        KeyCode::Down if numeric => Action::Increment(-step),
        KeyCode::Up => Action::ScrollUp,
        KeyCode::Down => Action::ScrollDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::Top,
        KeyCode::End => Action::Bottom,
        _ => return None,
    })
}

fn is_edit(action: Action) -> bool {
    matches!(
        action,
        Action::EditChar(_) | Action::EditBackspace | Action::EditCommit | Action::EditCancel
    )
}

/// Config can only change while idle, and `Parallel` only without auto mode.
fn field_editable(app: &App, focus: Focus) -> bool {
    match focus {
        Focus::Sims => !app.is_running,
        Focus::Parallel => !app.is_running && !app.config.auto_parallel,
        _ => false,
    }
}

fn field_value(app: &App, focus: Focus) -> i64 {
    match focus {
        Focus::Parallel => app.config.parallel_games,
        _ => app.config.simulations,
    }
}

fn set_field(app: &mut App, focus: Focus, value: i64) {
    app.set_config(|config| match focus {
        Focus::Sims => config.simulations = value,
        Focus::Parallel => config.parallel_games = value,
        _ => {}
    });
}

/// Applies the edited text; empty or invalid input keeps the previous value.
fn commit_edit(app: &mut App, view: &mut ViewState) {
    if let Some(buffer) = view.editing.take()
        && let Ok(value) = buffer.parse::<i64>()
        && field_editable(app, view.focus)
    {
        set_field(app, view.focus, value);
    }
}

fn start_stop(app: &mut App) {
    if app.is_running {
        app.stop();
    } else {
        app.start();
    }
}

fn open_export(app: &mut App, view: &mut ViewState) {
    if app.all_results.is_empty() {
        app.status_text = "No results to export".into();
        return;
    }
    let dir = std::env::current_dir().unwrap_or_default();
    let path = dir.join(app.default_export_file_name());
    view.export = Some(ExportDialog::new(path.display().to_string()));
}

/// Expands a leading `~` to the home directory.
fn expand_home(raw: &str) -> PathBuf {
    let home = || directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
    if raw == "~" {
        if let Some(home) = home() {
            return home;
        }
    } else if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
        && let Some(home) = home()
    {
        return home.join(rest);
    }
    PathBuf::from(raw)
}

fn dialog_key(code: KeyCode, app: &mut App, view: &mut ViewState) {
    let Some(dialog) = view.export.as_mut() else {
        return;
    };
    if dialog.confirm_overwrite {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => save_export(app, view),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                dialog.confirm_overwrite = false;
            }
            _ => {}
        }
        return;
    }
    dialog.error = None;
    match code {
        KeyCode::Esc => view.export = None,
        KeyCode::Enter => {
            if dialog.path.value().trim().is_empty() {
                return;
            }
            if expand_home(dialog.path.value()).exists() {
                dialog.confirm_overwrite = true;
            } else {
                save_export(app, view);
            }
        }
        KeyCode::Char(c) => dialog.path.insert(c),
        KeyCode::Backspace => dialog.path.backspace(),
        KeyCode::Delete => dialog.path.delete(),
        KeyCode::Left => dialog.path.left(),
        KeyCode::Right => dialog.path.right(),
        KeyCode::Home => dialog.path.home(),
        KeyCode::End => dialog.path.end(),
        _ => {}
    }
}

/// Writes the CSV; on failure the dialog stays open with the error so the
/// path can be fixed.
fn save_export(app: &mut App, view: &mut ViewState) {
    let Some(dialog) = view.export.as_mut() else {
        return;
    };
    let path = expand_home(dialog.path.value());
    match app.export_csv(&path) {
        Ok(()) => view.export = None,
        Err(err) => {
            dialog.confirm_overwrite = false;
            dialog.error = Some(format!("Error exporting CSV: {err}"));
        }
    }
}

pub fn apply(action: Action, app: &mut App, view: &mut ViewState) -> Flow {
    if view.editing.is_some() && !is_edit(action) {
        commit_edit(app, view);
    }
    let idle = !app.is_running;

    match action {
        Action::Quit => {
            if app.is_running {
                app.stop();
            }
            return Flow::Quit;
        }
        Action::FocusNext => view.focus = view.focus.next(),
        Action::FocusPrev => view.focus = view.focus.prev(),
        Action::Activate => match view.focus {
            Focus::Game => return apply(Action::GameNext, app, view),
            Focus::Sims | Focus::Parallel => {
                if field_editable(app, view.focus) {
                    view.editing = Some(field_value(app, view.focus).to_string());
                }
            }
            Focus::Auto => return apply(Action::ToggleAuto, app, view),
            Focus::StartStop => start_stop(app),
            Focus::Export => return apply(Action::Export, app, view),
            Focus::Clear => return apply(Action::Clear, app, view),
            Focus::Table => {}
        },
        Action::GameNext if idle => app.set_config(|c| c.game_type = c.game_type.next()),
        Action::GamePrev if idle => app.set_config(|c| c.game_type = c.game_type.prev()),
        Action::ToggleAuto if idle => app.set_config(|c| c.auto_parallel = !c.auto_parallel),
        Action::StartStop => start_stop(app),
        Action::Clear if can_clear(app) => app.clear_data(),
        Action::Export => open_export(app, view),
        Action::Dialog(code) => dialog_key(code, app, view),
        Action::EditChar(c) if field_editable(app, view.focus) => {
            let buffer = view.editing.get_or_insert_with(String::new);
            if buffer.len() < MAX_DIGITS {
                buffer.push(c);
            }
        }
        Action::EditBackspace if field_editable(app, view.focus) => {
            let current = field_value(app, view.focus).to_string();
            view.editing.get_or_insert(current).pop();
        }
        Action::EditCommit => commit_edit(app, view),
        Action::EditCancel => view.editing = None,
        Action::Increment(step) if field_editable(app, view.focus) => {
            let value = field_value(app, view.focus).saturating_add(step).max(0);
            set_field(app, view.focus, value);
        }
        Action::ScrollUp => view.table.select_previous(),
        Action::ScrollDown => view.table.select_next(),
        Action::PageUp => view.table.scroll_up_by(PAGE_ROWS),
        Action::PageDown => view.table.scroll_down_by(PAGE_ROWS),
        Action::Top => view.table.select_first(),
        Action::Bottom => view.table.select_last(),
        Action::SortNext => view.sort_column = view.sort_column.next(),
        Action::SortPrev => view.sort_column = view.sort_column.prev(),
        Action::ToggleHelp => view.show_help = !view.show_help,
        _ => {}
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{idle_app, result};
    use crate::model::{GameType, SimConfig, StopReason};
    use ratatui::crossterm::event::KeyEventState;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn view_at(focus: Focus) -> ViewState {
        let mut view = ViewState::new(false);
        view.focus = focus;
        view
    }

    fn type_keys(app: &mut App, view: &mut ViewState, keys: &[KeyCode]) {
        for &code in keys {
            if let Some(action) = map_key(press(code), view) {
                apply(action, app, view);
            }
        }
    }

    #[test]
    fn releases_are_ignored() {
        let view = view_at(Focus::Game);
        let release = KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        assert_eq!(map_key(release, &view), None);
    }

    #[test]
    fn key_map() {
        let game = view_at(Focus::Game);
        let sims = view_at(Focus::Sims);
        let table = view_at(Focus::Table);
        assert_eq!(map_key(press(KeyCode::Tab), &game), Some(Action::FocusNext));
        assert_eq!(
            map_key(press(KeyCode::Char('s')), &sims),
            Some(Action::StartStop)
        );
        assert_eq!(
            map_key(press(KeyCode::Char('7')), &sims),
            Some(Action::EditChar('7'))
        );
        assert_eq!(map_key(press(KeyCode::Char('7')), &game), None);
        assert_eq!(
            map_key(press(KeyCode::Right), &game),
            Some(Action::GameNext)
        );
        assert_eq!(
            map_key(press(KeyCode::Up), &sims),
            Some(Action::Increment(1))
        );
        assert_eq!(map_key(press(KeyCode::Up), &table), Some(Action::ScrollUp));
        assert_eq!(
            map_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &table
            ),
            Some(Action::Quit)
        );
    }

    #[test]
    fn help_swallows_other_keys() {
        let mut view = view_at(Focus::Game);
        view.show_help = true;
        assert_eq!(map_key(press(KeyCode::Char('s')), &view), None);
        assert_eq!(
            map_key(press(KeyCode::Esc), &view),
            Some(Action::ToggleHelp)
        );
    }

    #[test]
    fn typing_digits_and_enter_sets_simulations() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        let mut view = view_at(Focus::Sims);
        type_keys(
            &mut app,
            &mut view,
            &[KeyCode::Char('1'), KeyCode::Char('2'), KeyCode::Enter],
        );
        assert_eq!(app.config.simulations, 12);
        assert!(view.editing.is_none());
    }

    #[test]
    fn empty_input_and_escape_keep_previous_value() {
        let (mut app, _rx) = idle_app(SimConfig {
            simulations: 5,
            ..SimConfig::default()
        });
        let mut view = view_at(Focus::Sims);
        type_keys(&mut app, &mut view, &[KeyCode::Backspace, KeyCode::Enter]);
        assert_eq!(app.config.simulations, 5);

        type_keys(&mut app, &mut view, &[KeyCode::Char('9'), KeyCode::Esc]);
        assert_eq!(app.config.simulations, 5);
        assert!(view.editing.is_none());
    }

    #[test]
    fn moving_focus_commits_the_edit() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        let mut view = view_at(Focus::Parallel);
        type_keys(&mut app, &mut view, &[KeyCode::Char('3'), KeyCode::Tab]);
        assert_eq!(app.config.parallel_games, 3);
        assert_eq!(view.focus, Focus::Auto);
    }

    #[test]
    fn parallel_is_locked_in_auto_mode() {
        let (mut app, _rx) = idle_app(SimConfig {
            auto_parallel: true,
            parallel_games: 2,
            ..SimConfig::default()
        });
        let mut view = view_at(Focus::Parallel);
        type_keys(
            &mut app,
            &mut view,
            &[KeyCode::Char('8'), KeyCode::Enter, KeyCode::Up],
        );
        assert_eq!(app.config.parallel_games, 2);
    }

    #[test]
    fn config_is_locked_while_running() {
        let (mut app, _rx) = idle_app(SimConfig {
            simulations: 4,
            ..SimConfig::default()
        });
        let mut view = view_at(Focus::Sims);
        apply(Action::StartStop, &mut app, &mut view);
        assert!(app.is_running);

        type_keys(
            &mut app,
            &mut view,
            &[KeyCode::Char('9'), KeyCode::Enter, KeyCode::Up],
        );
        apply(Action::GameNext, &mut app, &mut view);
        apply(Action::ToggleAuto, &mut app, &mut view);
        assert_eq!(app.config.simulations, 4);
        assert_eq!(app.config.game_type, GameType::KlondikeDraw1);
        assert!(!app.config.auto_parallel);

        apply(Action::StartStop, &mut app, &mut view);
        assert!(!app.is_running);
        assert_eq!(app.status_text, "Stopped");
    }

    #[test]
    fn clear_respects_disabled_state() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        let mut view = view_at(Focus::Clear);
        apply(Action::Activate, &mut app, &mut view);
        assert_eq!(app.status_text, "Ready");

        app.record(result(1, true, StopReason::Win));
        apply(Action::Activate, &mut app, &mut view);
        assert_eq!(app.status_text, "Data cleared");
    }

    #[test]
    fn sort_and_help_toggle() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        let mut view = view_at(Focus::Table);
        apply(Action::SortNext, &mut app, &mut view);
        assert_eq!(view.sort_column, crate::ui::SortColumn::Moves);
        apply(Action::ToggleHelp, &mut app, &mut view);
        assert!(view.show_help);
    }

    #[test]
    fn export_without_results_only_sets_status() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        let mut view = view_at(Focus::Game);
        type_keys(&mut app, &mut view, &[KeyCode::Char('e')]);
        assert!(view.export.is_none());
        assert_eq!(app.status_text, "No results to export");
    }

    #[test]
    fn export_dialog_writes_and_confirms_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("results.csv");
        let (mut app, _rx) = idle_app(SimConfig::default());
        app.record(result(1, true, StopReason::Win));
        let mut view = view_at(Focus::Game);

        type_keys(&mut app, &mut view, &[KeyCode::Char('e')]);
        let dialog = view.export.as_mut().unwrap();
        assert!(dialog.path.value().ends_with(".csv"));
        dialog.path = crate::ui::dialogs::TextInput::new(path.display().to_string());

        // Keys go to the dialog, not to global shortcuts.
        type_keys(
            &mut app,
            &mut view,
            &[KeyCode::Char('s'), KeyCode::Backspace],
        );
        assert!(!app.is_running);
        type_keys(&mut app, &mut view, &[KeyCode::Enter]);
        assert!(view.export.is_none());
        assert_eq!(app.status_text, "CSV exported: results.csv");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), app.csv());

        // Second export to the same file asks before overwriting.
        type_keys(&mut app, &mut view, &[KeyCode::Char('e')]);
        view.export.as_mut().unwrap().path =
            crate::ui::dialogs::TextInput::new(path.display().to_string());
        type_keys(&mut app, &mut view, &[KeyCode::Enter]);
        assert!(view.export.as_ref().unwrap().confirm_overwrite);
        type_keys(&mut app, &mut view, &[KeyCode::Char('n')]);
        assert!(!view.export.as_ref().unwrap().confirm_overwrite);
        type_keys(&mut app, &mut view, &[KeyCode::Enter, KeyCode::Char('y')]);
        assert!(view.export.is_none());
    }

    #[test]
    fn export_dialog_keeps_open_on_error_and_esc_cancels() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "").unwrap();
        let (mut app, _rx) = idle_app(SimConfig::default());
        app.record(result(1, true, StopReason::Win));
        let mut view = view_at(Focus::Game);

        type_keys(&mut app, &mut view, &[KeyCode::Char('e')]);
        view.export.as_mut().unwrap().path =
            crate::ui::dialogs::TextInput::new(blocker.join("x.csv").display().to_string());
        type_keys(&mut app, &mut view, &[KeyCode::Enter]);
        assert!(view.export.as_ref().unwrap().error.is_some());
        assert_eq!(app.status_text, "Error exporting CSV");

        type_keys(&mut app, &mut view, &[KeyCode::Esc]);
        assert!(view.export.is_none());
    }

    #[test]
    fn expand_home_handles_tilde() {
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/a.csv"), home.join("a.csv"));
        assert_eq!(expand_home("/tmp/a.csv"), PathBuf::from("/tmp/a.csv"));
    }

    #[test]
    fn quit_stops_a_running_simulation() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        let mut view = view_at(Focus::Game);
        app.start();
        assert_eq!(apply(Action::Quit, &mut app, &mut view), Flow::Quit);
        assert!(!app.is_running);
    }

    #[test]
    fn full_run_through_the_real_runtime() {
        use crate::deck::ShuffleSource;
        use std::sync::mpsc;
        use std::time::Duration;

        let (tx, rx) = mpsc::channel();
        let config = SimConfig {
            game_type: GameType::TriPeaks,
            simulations: 2,
            parallel_games: 2,
            ..SimConfig::default()
        };
        let mut app = App::new(config, ShuffleSource::Seeded(3), tx);
        let mut view = view_at(Focus::StartStop);
        apply(Action::Activate, &mut app, &mut view);
        while app.is_running {
            let event = rx.recv_timeout(Duration::from_secs(60)).unwrap();
            app.handle_event(event);
        }
        assert_eq!(app.status_text, "Completed");
        assert_eq!(app.completed_games, 2);
        assert_eq!(app.all_results.len(), 2);
    }
}
