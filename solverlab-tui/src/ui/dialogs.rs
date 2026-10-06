//! CSV export dialog (replaces the Swift `NSSavePanel`).

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

/// Single-line text input with a cursor (in chars).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextInput {
    value: String,
    cursor: usize,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        TextInput { value, cursor }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    fn byte_index(&self, char_index: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_index)
            .map_or(self.value.len(), |(i, _)| i)
    }

    pub fn insert(&mut self, c: char) {
        let at = self.byte_index(self.cursor);
        self.value.insert(at, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let at = self.byte_index(self.cursor);
            self.value.remove(at);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.value.chars().count() {
            let at = self.byte_index(self.cursor);
            self.value.remove(at);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportDialog {
    pub path: TextInput,
    /// The target exists and the user is asked to confirm overwriting it.
    pub confirm_overwrite: bool,
    pub error: Option<String>,
}

impl ExportDialog {
    pub fn new(default_path: impl Into<String>) -> Self {
        ExportDialog {
            path: TextInput::new(default_path),
            confirm_overwrite: false,
            error: None,
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, dialog: &ExportDialog) {
    let width = area.width.saturating_sub(4).min(100);
    let popup = area.centered(Constraint::Length(width), Constraint::Length(7));
    frame.render_widget(Clear, popup);
    let block = Block::bordered().title(" Export CSV ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [label_area, input_area, message_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    frame.render_widget(Line::from("Save results to:"), label_area);

    // Scroll horizontally so the cursor stays visible.
    let visible = usize::from(input_area.width.max(1));
    let start = (dialog.path.cursor + 1).saturating_sub(visible);
    let shown: String = dialog
        .path
        .value
        .chars()
        .skip(start)
        .take(visible)
        .collect();
    frame.render_widget(
        Paragraph::new(shown).style(Style::default().add_modifier(Modifier::UNDERLINED)),
        input_area,
    );
    if !dialog.confirm_overwrite {
        let x = input_area.x + u16::try_from(dialog.path.cursor - start).unwrap_or(0);
        frame.set_cursor_position(Position::new(x, input_area.y));
    }

    if let Some(error) = &dialog.error {
        frame.render_widget(
            Line::styled(error.as_str(), Style::default().fg(Color::Red)),
            message_area,
        );
    } else if dialog.confirm_overwrite {
        frame.render_widget(
            Line::styled(
                "File exists. Overwrite? (y/n)",
                Style::default().fg(Color::Yellow),
            ),
            message_area,
        );
    }

    let hint = if dialog.confirm_overwrite {
        "y overwrite · n / Esc edit path"
    } else {
        "Enter save · Esc cancel"
    };
    frame.render_widget(
        Line::styled(hint, Style::default().add_modifier(Modifier::DIM)),
        hint_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_edits_at_cursor() {
        let mut input = TextInput::new("ab");
        input.left();
        input.insert('é');
        assert_eq!(input.value(), "aéb");
        input.home();
        input.delete();
        assert_eq!(input.value(), "éb");
        input.end();
        input.backspace();
        assert_eq!(input.value(), "é");
        input.right();
        input.backspace();
        input.backspace();
        assert_eq!(input.value(), "");
    }
}
