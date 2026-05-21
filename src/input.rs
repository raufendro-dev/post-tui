use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    value: String,
    cursor: usize,
}

impl TextInput {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn value_with_cursor(&self, cursor_visible: bool) -> String {
        let cursor = self.safe_cursor();
        let (left, right) = self.value.split_at(cursor);
        let cursor = if cursor_visible { "█" } else { " " };
        if self.value.is_empty() {
            cursor.to_string()
        } else {
            format!("{left}{cursor}{right}")
        }
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.len();
    }

    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.value)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = 0;
                true
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.value.len();
                true
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.clear();
                self.cursor = 0;
                true
            }
            KeyCode::Char(ch) => {
                self.value.insert(self.cursor, ch);
                self.cursor += ch.len_utf8();
                true
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.value.len());
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.value.len();
                true
            }
            _ => false,
        }
    }

    fn safe_cursor(&self) -> usize {
        let mut cursor = self.cursor.min(self.value.len());
        while cursor > 0 && !self.value.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
    }
}
