//! Core editor state and rendering types for minivim.

use std::fs;
use std::io;
use std::path::PathBuf;

use crossterm::event::Event;
use crossterm::style::ContentStyle;

/// Editor mode for key handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

/// Cursor position in the buffer (0-based).
#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

/// Viewport offsets into the buffer for rendering.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub row_offset: usize,
    pub col_offset: usize,
}

/// In-memory text buffer stored as lines.
#[derive(Debug, Clone)]
pub struct Buffer {
    pub lines: Vec<String>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
        }
    }

    pub fn from_string(contents: String) -> Self {
        let mut lines: Vec<String> = contents.split('\n').map(|line| line.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self { lines }
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }
}

/// State for ex-style command input.
#[derive(Debug, Clone)]
pub struct CommandLine {
    pub active: bool,
    pub input: String,
}

impl CommandLine {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
        }
    }
}

/// Shared editor state used by plugins.
#[derive(Debug)]
pub struct Editor {
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub viewport: Viewport,
    pub mode: Mode,
    pub command_line: CommandLine,
    pub status: String,
    pub file_path: Option<PathBuf>,
    pub should_quit: bool,
    pub dirty: bool,
    pub revision: u64,
    pub screen_width: u16,
    pub screen_height: u16,
    command_queue: Vec<String>,
}

impl Editor {
    pub fn new(screen_width: u16, screen_height: u16, file_path: Option<PathBuf>) -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor { row: 0, col: 0 },
            viewport: Viewport {
                row_offset: 0,
                col_offset: 0,
            },
            mode: Mode::Normal,
            command_line: CommandLine::new(),
            status: String::new(),
            file_path,
            should_quit: false,
            dirty: false,
            revision: 0,
            screen_width,
            screen_height,
            command_queue: Vec::new(),
        }
    }

    pub fn set_screen_size(&mut self, width: u16, height: u16) {
        self.screen_width = width;
        self.screen_height = height;
        self.ensure_cursor_visible();
    }

    pub fn content_height(&self) -> u16 {
        let gutter = if self.command_line.active { 2 } else { 1 };
        self.screen_height.saturating_sub(gutter)
    }

    pub fn status_row(&self) -> u16 {
        if self.command_line.active {
            self.screen_height.saturating_sub(2)
        } else {
            self.screen_height.saturating_sub(1)
        }
    }

    pub fn command_row(&self) -> u16 {
        self.screen_height.saturating_sub(1)
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
    }

    pub fn push_command(&mut self, command: String) {
        self.command_queue.push(command);
    }

    pub fn take_commands(&mut self) -> Vec<String> {
        std::mem::take(&mut self.command_queue)
    }

    pub fn load_from_path(&mut self, path: &PathBuf) -> io::Result<()> {
        let contents = fs::read_to_string(path)?;
        self.buffer = Buffer::from_string(contents);
        self.cursor = Cursor { row: 0, col: 0 };
        self.viewport = Viewport {
            row_offset: 0,
            col_offset: 0,
        };
        self.dirty = false;
        self.revision = 0;
        Ok(())
    }

    pub fn save_to_path(&mut self, path: &PathBuf) -> io::Result<()> {
        fs::write(path, self.buffer.to_string())?;
        self.dirty = false;
        Ok(())
    }

    pub fn current_line_len(&self) -> usize {
        self.buffer
            .lines
            .get(self.cursor.row)
            .map(|line| line.chars().count())
            .unwrap_or(0)
    }

    pub fn clamp_cursor(&mut self) {
        if self.cursor.row >= self.buffer.lines.len() {
            self.cursor.row = self.buffer.lines.len().saturating_sub(1);
            self.cursor.col = 0;
        }
        let line_len = self.current_line_len();
        if self.cursor.col > line_len {
            self.cursor.col = line_len;
        }
    }

    pub fn ensure_cursor_visible(&mut self) {
        let content_height = self.content_height() as usize;
        if content_height == 0 {
            self.viewport.row_offset = self.cursor.row;
        } else if self.cursor.row < self.viewport.row_offset {
            self.viewport.row_offset = self.cursor.row;
        } else if self.cursor.row >= self.viewport.row_offset + content_height {
            self.viewport.row_offset = self.cursor.row.saturating_sub(content_height - 1);
        }

        let content_width = self.screen_width as usize;
        if content_width == 0 {
            self.viewport.col_offset = self.cursor.col;
        } else if self.cursor.col < self.viewport.col_offset {
            self.viewport.col_offset = self.cursor.col;
        } else if self.cursor.col >= self.viewport.col_offset + content_width {
            self.viewport.col_offset = self.cursor.col.saturating_sub(content_width - 1);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        }
        self.ensure_cursor_visible();
    }

    pub fn move_right(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        }
        self.ensure_cursor_visible();
    }

    pub fn move_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.clamp_cursor();
        }
        self.ensure_cursor_visible();
    }

    pub fn move_down(&mut self) {
        if self.cursor.row + 1 < self.buffer.lines.len() {
            self.cursor.row += 1;
            self.clamp_cursor();
        }
        self.ensure_cursor_visible();
    }

    pub fn move_line_start(&mut self) {
        self.cursor.col = 0;
        self.ensure_cursor_visible();
    }

    pub fn move_line_end(&mut self) {
        self.cursor.col = self.current_line_len();
        self.ensure_cursor_visible();
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.cursor.row >= self.buffer.lines.len() {
            self.buffer.lines.push(String::new());
        }
        let line = &mut self.buffer.lines[self.cursor.row];
        let byte_idx = Self::char_to_byte_index(line, self.cursor.col);
        line.insert(byte_idx, ch);
        self.cursor.col += 1;
        self.dirty = true;
        self.bump_revision();
        self.ensure_cursor_visible();
    }

    pub fn insert_newline(&mut self) {
        if self.cursor.row >= self.buffer.lines.len() {
            self.buffer.lines.push(String::new());
        }
        let line = &mut self.buffer.lines[self.cursor.row];
        let byte_idx = Self::char_to_byte_index(line, self.cursor.col);
        let new_line = line.split_off(byte_idx);
        self.buffer.lines.insert(self.cursor.row + 1, new_line);
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.dirty = true;
        self.bump_revision();
        self.ensure_cursor_visible();
    }

    pub fn backspace(&mut self) {
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }
        if self.cursor.col > 0 {
            let line = &mut self.buffer.lines[self.cursor.row];
            let remove_col = self.cursor.col - 1;
            let byte_idx = Self::char_to_byte_index(line, remove_col);
            line.remove(byte_idx);
            self.cursor.col -= 1;
            self.dirty = true;
            self.bump_revision();
        } else if self.cursor.row > 0 {
            let current = self.buffer.lines.remove(self.cursor.row);
            self.cursor.row -= 1;
            let line = &mut self.buffer.lines[self.cursor.row];
            let prev_len = line.len();
            line.push_str(&current);
            self.cursor.col = prev_len;
            self.dirty = true;
            self.bump_revision();
        }
        self.ensure_cursor_visible();
    }

    pub fn delete_char(&mut self) {
        if self.cursor.row >= self.buffer.lines.len() {
            return;
        }
        let line_len = self.current_line_len();
        if self.cursor.col < line_len {
            let line = &mut self.buffer.lines[self.cursor.row];
            let byte_idx = Self::char_to_byte_index(line, self.cursor.col);
            line.remove(byte_idx);
            self.dirty = true;
            self.bump_revision();
        } else if self.cursor.row + 1 < self.buffer.lines.len() {
            let next = self.buffer.lines.remove(self.cursor.row + 1);
            let line = &mut self.buffer.lines[self.cursor.row];
            line.push_str(&next);
            self.dirty = true;
            self.bump_revision();
        }
        self.ensure_cursor_visible();
    }

    fn char_to_byte_index(line: &str, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        line.char_indices()
            .nth(char_index)
            .map(|(idx, _)| idx)
            .unwrap_or_else(|| line.len())
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

/// Result of handling an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Consumed,
    Ignored,
}

/// Plugin interface for extending editor behavior.
pub trait Plugin {
    fn on_init(&mut self, _editor: &mut Editor) {}

    fn on_event(&mut self, _editor: &mut Editor, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn on_command(&mut self, _editor: &mut Editor, _command: &str) -> EventResult {
        EventResult::Ignored
    }

    fn on_render(&mut self, _editor: &Editor, _ctx: &mut RenderContext) {}
}

/// Render buffer used by plugins to draw UI content.
pub struct RenderContext {
    pub width: u16,
    pub height: u16,
    pub lines: Vec<String>,
    pub spans: Vec<Vec<StyledSpan>>,
    pub cursor: Option<(u16, u16)>,
}

impl RenderContext {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            lines: vec![String::new(); height as usize],
            spans: vec![Vec::new(); height as usize],
            cursor: None,
        }
    }

    pub fn set_line(&mut self, row: u16, text: String) {
        let row_index = row as usize;
        if row_index >= self.lines.len() {
            return;
        }
        let max_width = self.width as usize;
        if max_width == 0 {
            self.lines[row_index] = String::new();
            return;
        }
        let line: String = text.chars().take(max_width).collect();
        self.lines[row_index] = line;
    }

    pub fn set_spans(&mut self, row: u16, spans: Vec<StyledSpan>) {
        let row_index = row as usize;
        if row_index >= self.spans.len() {
            return;
        }
        self.spans[row_index] = spans;
    }

    pub fn set_cursor(&mut self, row: u16, col: u16) {
        self.cursor = Some((row, col));
    }
}

/// Styled span in a rendered line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledSpan {
    pub start: usize,
    pub len: usize,
    pub style: ContentStyle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_from_string_preserves_trailing_line() {
        let buffer = Buffer::from_string("a\nb\n".to_string());
        assert_eq!(buffer.lines, vec!["a", "b", ""]);
    }

    #[test]
    fn insert_newline_splits_line() {
        let mut editor = Editor::new(80, 24, None);
        editor.buffer.lines = vec!["hello".to_string()];
        editor.cursor.row = 0;
        editor.cursor.col = 2;
        editor.insert_newline();
        assert_eq!(editor.buffer.lines, vec!["he", "llo"]);
        assert_eq!(editor.cursor.row, 1);
        assert_eq!(editor.cursor.col, 0);
    }

    #[test]
    fn backspace_merges_lines_at_start() {
        let mut editor = Editor::new(80, 24, None);
        editor.buffer.lines = vec!["hi".to_string(), "there".to_string()];
        editor.cursor.row = 1;
        editor.cursor.col = 0;
        editor.backspace();
        assert_eq!(editor.buffer.lines, vec!["hithere"]);
        assert_eq!(editor.cursor.row, 0);
        assert_eq!(editor.cursor.col, 2);
    }

    #[test]
    fn delete_char_merges_lines_at_end() {
        let mut editor = Editor::new(80, 24, None);
        editor.buffer.lines = vec!["hi".to_string(), "there".to_string()];
        editor.cursor.row = 0;
        editor.cursor.col = 2;
        editor.delete_char();
        assert_eq!(editor.buffer.lines, vec!["hithere"]);
        assert_eq!(editor.cursor.row, 0);
        assert_eq!(editor.cursor.col, 2);
    }

    #[test]
    fn revision_increments_on_edits() {
        let mut editor = Editor::new(80, 24, None);
        assert_eq!(editor.revision, 0);
        editor.insert_char('a');
        let after_insert = editor.revision;
        editor.insert_newline();
        let after_newline = editor.revision;
        editor.backspace();
        let after_backspace = editor.revision;
        assert!(after_insert > 0);
        assert!(after_newline > after_insert);
        assert!(after_backspace > after_newline);
    }

    #[test]
    fn clamp_cursor_trims_column() {
        let mut editor = Editor::new(80, 24, None);
        editor.buffer.lines = vec!["hi".to_string()];
        editor.cursor.row = 0;
        editor.cursor.col = 10;
        editor.clamp_cursor();
        assert_eq!(editor.cursor.col, 2);
    }
}
