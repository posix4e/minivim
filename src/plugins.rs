use std::io;
use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use crossterm::style::{Attribute, Attributes, Color, ContentStyle};

use syntect::easy::HighlightLines;
use syntect::highlighting::{Color as SyntectColor, FontStyle, Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use crate::editor::{Editor, EventResult, Mode, Plugin, RenderContext, StyledSpan};

pub struct FileCommandPlugin;

impl FileCommandPlugin {
    fn save_to_path(editor: &mut Editor, path: PathBuf) -> bool {
        match editor.save_to_path(&path) {
            Ok(()) => {
                editor.file_path = Some(path.clone());
                editor.set_status(format!("Wrote {}", path.display()));
                true
            }
            Err(err) => {
                editor.set_status(format!("Write failed: {}", err));
                false
            }
        }
    }

    fn command_quit(editor: &mut Editor, force: bool) {
        if editor.dirty && !force {
            editor.set_status("No write since last change (add ! to override)");
        } else {
            editor.should_quit = true;
        }
    }
}

impl Plugin for FileCommandPlugin {
    fn on_init(&mut self, editor: &mut Editor) {
        let Some(path) = editor.file_path.clone() else {
            return;
        };
        match editor.load_from_path(&path) {
            Ok(()) => editor.set_status(format!("Opened {}", path.display())),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    editor.set_status(format!("New file {}", path.display()));
                } else {
                    editor.set_status(format!("Open failed: {}", err));
                }
            }
        }
    }

    fn on_command(&mut self, editor: &mut Editor, command: &str) -> EventResult {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return EventResult::Consumed;
        }

        let mut parts = trimmed.split_whitespace();
        let verb = parts.next().unwrap_or("");

        match verb {
            "w" => {
                let path = parts
                    .next()
                    .map(PathBuf::from)
                    .or_else(|| editor.file_path.clone());
                if let Some(path) = path {
                    Self::save_to_path(editor, path);
                } else {
                    editor.set_status("No file name");
                }
                EventResult::Consumed
            }
            "wq" | "x" => {
                let path = parts
                    .next()
                    .map(PathBuf::from)
                    .or_else(|| editor.file_path.clone());
                if let Some(path) = path {
                    if Self::save_to_path(editor, path) {
                        editor.should_quit = true;
                    }
                } else {
                    editor.set_status("No file name");
                }
                EventResult::Consumed
            }
            "q" => {
                Self::command_quit(editor, false);
                EventResult::Consumed
            }
            "q!" => {
                Self::command_quit(editor, true);
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

pub struct ModePlugin;

impl Plugin for ModePlugin {
    fn on_event(&mut self, editor: &mut Editor, event: &Event) -> EventResult {
        let Event::Key(key) = event else {
            return EventResult::Ignored;
        };

        match key.code {
            KeyCode::Esc => {
                editor.mode = Mode::Normal;
                editor.command_line.active = false;
                editor.command_line.input.clear();
                EventResult::Consumed
            }
            KeyCode::Char('i') if editor.mode == Mode::Normal => {
                editor.mode = Mode::Insert;
                EventResult::Consumed
            }
            KeyCode::Char(':') if editor.mode == Mode::Normal => {
                editor.mode = Mode::Command;
                editor.command_line.active = true;
                editor.command_line.input.clear();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

pub struct CommandLinePlugin;

impl Plugin for CommandLinePlugin {
    fn on_event(&mut self, editor: &mut Editor, event: &Event) -> EventResult {
        if editor.mode != Mode::Command {
            return EventResult::Ignored;
        }
        let Event::Key(key) = event else {
            return EventResult::Ignored;
        };

        match key.code {
            KeyCode::Enter => {
                let command = editor.command_line.input.trim().to_string();
                editor.command_line.input.clear();
                editor.command_line.active = false;
                editor.mode = Mode::Normal;
                if !command.is_empty() {
                    editor.push_command(command);
                }
                EventResult::Consumed
            }
            KeyCode::Backspace => {
                editor.command_line.input.pop();
                EventResult::Consumed
            }
            KeyCode::Char(ch) => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    return EventResult::Ignored;
                }
                editor.command_line.input.push(ch);
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

pub struct MotionPlugin;

impl Plugin for MotionPlugin {
    fn on_event(&mut self, editor: &mut Editor, event: &Event) -> EventResult {
        if editor.mode != Mode::Normal {
            return EventResult::Ignored;
        }
        let Event::Key(key) = event else {
            return EventResult::Ignored;
        };

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return EventResult::Ignored;
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => {
                editor.move_left();
                EventResult::Consumed
            }
            KeyCode::Char('l') | KeyCode::Right => {
                editor.move_right();
                EventResult::Consumed
            }
            KeyCode::Char('k') | KeyCode::Up => {
                editor.move_up();
                EventResult::Consumed
            }
            KeyCode::Char('j') | KeyCode::Down => {
                editor.move_down();
                EventResult::Consumed
            }
            KeyCode::Char('0') => {
                editor.move_line_start();
                EventResult::Consumed
            }
            KeyCode::Char('$') => {
                editor.move_line_end();
                EventResult::Consumed
            }
            KeyCode::Char('x') => {
                editor.delete_char();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

pub struct InsertPlugin;

impl Plugin for InsertPlugin {
    fn on_event(&mut self, editor: &mut Editor, event: &Event) -> EventResult {
        if editor.mode != Mode::Insert {
            return EventResult::Ignored;
        }
        let Event::Key(key) = event else {
            return EventResult::Ignored;
        };

        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return EventResult::Ignored;
        }

        match key.code {
            KeyCode::Char(ch) => {
                editor.insert_char(ch);
                EventResult::Consumed
            }
            KeyCode::Enter => {
                editor.insert_newline();
                EventResult::Consumed
            }
            KeyCode::Backspace => {
                editor.backspace();
                EventResult::Consumed
            }
            KeyCode::Delete => {
                editor.delete_char();
                EventResult::Consumed
            }
            KeyCode::Tab => {
                for _ in 0..4 {
                    editor.insert_char(' ');
                }
                EventResult::Consumed
            }
            KeyCode::Left => {
                editor.move_left();
                EventResult::Consumed
            }
            KeyCode::Right => {
                editor.move_right();
                EventResult::Consumed
            }
            KeyCode::Up => {
                editor.move_up();
                EventResult::Consumed
            }
            KeyCode::Down => {
                editor.move_down();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

pub struct BufferRenderPlugin;

impl Plugin for BufferRenderPlugin {
    fn on_render(&mut self, editor: &Editor, ctx: &mut RenderContext) {
        let content_height = editor.content_height();
        let width = ctx.width as usize;
        for row in 0..content_height {
            let buffer_row = editor.viewport.row_offset + row as usize;
            if buffer_row < editor.buffer.lines.len() {
                let line = &editor.buffer.lines[buffer_row];
                let slice = slice_line(line, editor.viewport.col_offset, width);
                ctx.set_line(row, slice);
            } else {
                ctx.set_line(row, "~".to_string());
            }
        }
    }
}

pub struct SyntaxHighlightPlugin {
    syntax_set: SyntaxSet,
    theme: Theme,
    cached_spans: Vec<Vec<StyledSpan>>,
    last_revision: u64,
    last_path: Option<PathBuf>,
}

impl SyntaxHighlightPlugin {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .or_else(|| theme_set.themes.values().next().cloned())
            .expect("syntect themes are missing");

        Self {
            syntax_set,
            theme,
            cached_spans: Vec::new(),
            last_revision: u64::MAX,
            last_path: None,
        }
    }

    fn needs_rehighlight(&self, editor: &Editor) -> bool {
        editor.revision != self.last_revision
            || editor.file_path != self.last_path
            || editor.buffer.lines.len() != self.cached_spans.len()
    }

    fn syntax_for_editor(&self, editor: &Editor) -> &SyntaxReference {
        if let Some(path) = editor.file_path.as_ref() {
            if let Ok(Some(syntax)) = self.syntax_set.find_syntax_for_file(path) {
                return syntax;
            }
        }
        self.syntax_set.find_syntax_plain_text()
    }

    fn rehighlight(&mut self, editor: &Editor) {
        let syntax = self.syntax_for_editor(editor);
        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut spans = Vec::with_capacity(editor.buffer.lines.len());

        for (idx, line) in editor.buffer.lines.iter().enumerate() {
            let mut owned = line.clone();
            if idx + 1 < editor.buffer.lines.len() {
                owned.push('\n');
            }
            let ranges = match highlighter.highlight_line(&owned, &self.syntax_set) {
                Ok(ranges) => ranges,
                Err(_) => Vec::new(),
            };
            let line_spans = Self::spans_from_ranges(&ranges);
            spans.push(line_spans);
        }

        self.cached_spans = spans;
        self.last_revision = editor.revision;
        self.last_path = editor.file_path.clone();
    }

    fn spans_from_ranges(ranges: &[(Style, &str)]) -> Vec<StyledSpan> {
        let mut spans: Vec<StyledSpan> = Vec::new();
        let mut col = 0usize;

        for (style, text) in ranges {
            let mut len = 0usize;
            for ch in text.chars() {
                if ch == '\n' || ch == '\r' {
                    break;
                }
                len += 1;
            }
            if len == 0 {
                continue;
            }

            let content_style = Self::map_style(*style);
            if let Some(last) = spans.last_mut() {
                if last.style == content_style && last.start + last.len == col {
                    last.len += len;
                    col += len;
                    continue;
                }
            }

            spans.push(StyledSpan {
                start: col,
                len,
                style: content_style,
            });
            col += len;
        }

        spans
    }

    fn map_style(style: Style) -> ContentStyle {
        let mut content = ContentStyle::new();
        content.foreground_color = Self::map_color(style.foreground);
        content.background_color = Self::map_color(style.background);
        let mut attrs = Attributes::default();
        if style.font_style.contains(FontStyle::BOLD) {
            attrs.set(Attribute::Bold);
        }
        if style.font_style.contains(FontStyle::ITALIC) {
            attrs.set(Attribute::Italic);
        }
        if style.font_style.contains(FontStyle::UNDERLINE) {
            attrs.set(Attribute::Underlined);
        }
        content.attributes = attrs;
        content
    }

    fn map_color(color: SyntectColor) -> Option<Color> {
        if color.a == 0 {
            None
        } else {
            Some(Color::Rgb {
                r: color.r,
                g: color.g,
                b: color.b,
            })
        }
    }

    fn slice_spans(spans: &[StyledSpan], col_offset: usize, width: usize) -> Vec<StyledSpan> {
        if width == 0 {
            return Vec::new();
        }
        let end = col_offset.saturating_add(width);
        let mut visible = Vec::new();
        for span in spans {
            let span_start = span.start;
            let span_end = span.start + span.len;
            if span_end <= col_offset || span_start >= end {
                continue;
            }
            let start = span_start.max(col_offset) - col_offset;
            let end = span_end.min(end) - col_offset;
            let len = end.saturating_sub(start);
            if len == 0 {
                continue;
            }
            visible.push(StyledSpan {
                start,
                len,
                style: span.style,
            });
        }
        visible
    }
}

impl Plugin for SyntaxHighlightPlugin {
    fn on_render(&mut self, editor: &Editor, ctx: &mut RenderContext) {
        if self.needs_rehighlight(editor) {
            self.rehighlight(editor);
        }

        let width = ctx.width as usize;
        let content_height = editor.content_height();
        for row in 0..content_height {
            let buffer_row = editor.viewport.row_offset + row as usize;
            if buffer_row >= self.cached_spans.len() {
                continue;
            }
            let spans = Self::slice_spans(
                &self.cached_spans[buffer_row],
                editor.viewport.col_offset,
                width,
            );
            ctx.set_spans(row, spans);
        }
    }
}

pub struct StatusBarPlugin;

impl Plugin for StatusBarPlugin {
    fn on_render(&mut self, editor: &Editor, ctx: &mut RenderContext) {
        if ctx.height == 0 {
            return;
        }

        let mode_label = match editor.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
        };

        let name = editor
            .file_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "[No Name]".to_string());
        let dirty = if editor.dirty { " [+]" } else { "" };

        let left = format!("{} {}{}", mode_label, name, dirty);
        let right = if editor.status.is_empty() {
            format!(
                "Ln {}, Col {}",
                editor.cursor.row + 1,
                editor.cursor.col + 1
            )
        } else {
            editor.status.clone()
        };

        let line = format_status_line(&left, &right, ctx.width as usize);
        ctx.set_line(editor.status_row(), line);
    }
}

pub struct CommandLineRenderPlugin;

impl Plugin for CommandLineRenderPlugin {
    fn on_render(&mut self, editor: &Editor, ctx: &mut RenderContext) {
        if !editor.command_line.active || ctx.height == 0 {
            return;
        }
        let prompt = format!(":{}", editor.command_line.input);
        ctx.set_line(editor.command_row(), prompt);
    }
}

pub struct CursorRenderPlugin;

impl Plugin for CursorRenderPlugin {
    fn on_render(&mut self, editor: &Editor, ctx: &mut RenderContext) {
        if ctx.height == 0 || ctx.width == 0 {
            return;
        }
        if editor.command_line.active {
            let row = editor.command_row().min(ctx.height.saturating_sub(1));
            let col = (1 + editor.command_line.input.chars().count()) as u16;
            let clamped = col.min(ctx.width.saturating_sub(1));
            ctx.set_cursor(row, clamped);
            return;
        }

        let cursor_row = editor.cursor.row.saturating_sub(editor.viewport.row_offset) as u16;
        let cursor_col = editor.cursor.col.saturating_sub(editor.viewport.col_offset) as u16;
        let row = cursor_row.min(ctx.height.saturating_sub(1));
        let col = cursor_col.min(ctx.width.saturating_sub(1));
        ctx.set_cursor(row, col);
    }
}

fn slice_line(line: &str, col_offset: usize, width: usize) -> String {
    line.chars()
        .skip(col_offset)
        .take(width)
        .collect::<String>()
}

fn format_status_line(left: &str, right: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let right_len = right.chars().count();

    if right_len >= width {
        return right.chars().take(width).collect();
    }

    let available_left = width.saturating_sub(right_len + 1);
    let left_trimmed: String = left.chars().take(available_left).collect();
    let padding = width.saturating_sub(left_trimmed.chars().count() + right_len);
    format!("{}{}{}", left_trimmed, " ".repeat(padding), right)
}
