use std::io::{self, Write};
use std::path::PathBuf;

use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

mod editor;
mod plugins;

use editor::{Editor, EventResult, Plugin, RenderContext};
use plugins::{
    BufferRenderPlugin, CommandLinePlugin, CommandLineRenderPlugin, CursorRenderPlugin,
    FileCommandPlugin, InsertPlugin, ModePlugin, MotionPlugin, StatusBarPlugin,
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
        let mut rendered = line.clone();
        let width = ctx.width as usize;
        let line_len = rendered.chars().count();
        if width > line_len {
            rendered.push_str(&" ".repeat(width - line_len));
        } else if width > 0 && line_len > width {
            rendered = rendered.chars().take(width).collect();
        }
        queue!(stdout, crossterm::style::Print(rendered))?;
    }

    if let Some((row, col)) = ctx.cursor {
        queue!(stdout, cursor::MoveTo(col, row), cursor::Show)?;
    } else {
        queue!(stdout, cursor::Hide)?;
    }

    stdout.flush()
}
