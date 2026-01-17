use std::io::{self, Write};
use std::path::PathBuf;

use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    style::{Print, PrintStyledContent},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

mod editor;
mod plugins;

use editor::{Editor, EventResult, Plugin, RenderContext, StyledSpan};
use plugins::{
    BufferRenderPlugin, CommandLinePlugin, CommandLineRenderPlugin, CursorRenderPlugin,
    FileCommandPlugin, InsertPlugin, ModePlugin, MotionPlugin, StatusBarPlugin,
    SyntaxHighlightPlugin,
};

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

fn main() -> io::Result<()> {
    let _terminal = TerminalGuard::new()?;
    let (width, height) = terminal::size()?;
    let file_path = std::env::args().nth(1).map(PathBuf::from);
    let mut editor = Editor::new(width, height, file_path);

    let mut plugins: Vec<Box<dyn Plugin>> = vec![
        Box::new(FileCommandPlugin),
        Box::new(ModePlugin),
        Box::new(CommandLinePlugin),
        Box::new(MotionPlugin),
        Box::new(InsertPlugin),
        Box::new(BufferRenderPlugin),
        Box::new(SyntaxHighlightPlugin::new()),
        Box::new(StatusBarPlugin),
        Box::new(CommandLineRenderPlugin),
        Box::new(CursorRenderPlugin),
    ];

    for plugin in plugins.iter_mut() {
        plugin.on_init(&mut editor);
    }

    render(&editor, &mut plugins)?;

    loop {
        let event = event::read()?;
        if let Event::Resize(width, height) = event {
            editor.set_screen_size(width, height);
        }

        for plugin in plugins.iter_mut() {
            if plugin.on_event(&mut editor, &event) == EventResult::Consumed {
                break;
            }
        }

        for command in editor.take_commands() {
            for plugin in plugins.iter_mut() {
                if plugin.on_command(&mut editor, &command) == EventResult::Consumed {
                    break;
                }
            }
        }

        if editor.should_quit {
            break;
        }

        render(&editor, &mut plugins)?;
    }

    Ok(())
}

fn render(editor: &Editor, plugins: &mut [Box<dyn Plugin>]) -> io::Result<()> {
    let mut ctx = RenderContext::new(editor.screen_width, editor.screen_height);
    for plugin in plugins.iter_mut() {
        plugin.on_render(editor, &mut ctx);
    }

    let mut stdout = io::stdout();
    queue!(stdout, cursor::Hide, Clear(ClearType::All))?;
    for (row, line) in ctx.lines.iter().enumerate() {
        queue!(
            stdout,
            cursor::MoveTo(0, row as u16),
            Clear(ClearType::CurrentLine)
        )?;
        let spans = ctx.spans.get(row).map(Vec::as_slice).unwrap_or(&[]);
        render_line(&mut stdout, line, spans, ctx.width as usize)?;
    }

    if let Some((row, col)) = ctx.cursor {
        queue!(stdout, cursor::MoveTo(col, row), cursor::Show)?;
    } else {
        queue!(stdout, cursor::Hide)?;
    }

    stdout.flush()
}

fn render_line(
    stdout: &mut impl Write,
    line: &str,
    spans: &[StyledSpan],
    width: usize,
) -> io::Result<()> {
    let mut line_chars: Vec<char> = line.chars().collect();
    if width == 0 {
        return Ok(());
    }
    if line_chars.len() > width {
        line_chars.truncate(width);
    }

    if spans.is_empty() {
        let rendered: String = line_chars.iter().collect();
        queue!(stdout, Print(rendered))?;
        let padding = width.saturating_sub(line_chars.len());
        if padding > 0 {
            queue!(stdout, Print(" ".repeat(padding)))?;
        }
        return Ok(());
    }

    let mut spans_sorted = spans.to_vec();
    spans_sorted.sort_by_key(|span| span.start);
    let mut cursor = 0usize;
    let line_len = line_chars.len();

    for span in spans_sorted {
        let span_start = span.start.min(width).min(line_len);
        if span_start > cursor {
            let segment: String = line_chars[cursor..span_start].iter().collect();
            queue!(stdout, Print(segment))?;
        }

        let span_end = span.start.saturating_add(span.len).min(width).min(line_len);
        if span_end > span_start {
            let segment: String = line_chars[span_start..span_end].iter().collect();
            queue!(stdout, PrintStyledContent(span.style.apply(segment)))?;
        }
        cursor = span_end;
    }

    if cursor < line_len {
        let segment: String = line_chars[cursor..line_len].iter().collect();
        queue!(stdout, Print(segment))?;
    }

    let padding = width.saturating_sub(line_len);
    if padding > 0 {
        queue!(stdout, Print(" ".repeat(padding)))?;
    }

    Ok(())
}
