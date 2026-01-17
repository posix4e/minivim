#![cfg(unix)]

use expectrl::{Eof, Session};
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use std::{thread};
use tempfile::tempdir;
use vt100::Parser;

fn spawn_minivim(path: &Path) -> Session {
    let bin = assert_cmd::cargo::cargo_bin!("minivim");
    let mut cmd = Command::new(bin);
    cmd.arg(path);
    cmd.env("TERM", "xterm-256color");
    Session::spawn(cmd).expect("spawn minivim")
}

fn drain_output(session: &mut Session, parser: &mut Parser, duration: Duration) -> io::Result<()> {
    let start = Instant::now();
    let mut buf = [0u8; 8192];
    while start.elapsed() < duration {
        match session.try_read(&mut buf) {
            Ok(0) => thread::sleep(Duration::from_millis(10)),
            Ok(n) => parser.process(&buf[..n]),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn wait_for_text(
    session: &mut Session,
    parser: &mut Parser,
    text: &str,
    timeout: Duration,
) -> io::Result<bool> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        drain_output(session, parser, Duration::from_millis(50))?;
        if parser.screen().contents().contains(text) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[test]
fn insert_and_write() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("note.txt");

    let mut session = spawn_minivim(&path);
    session.set_expect_timeout(Some(Duration::from_secs(2)));

    let mut parser = Parser::new(24, 80, 0);
    drain_output(&mut session, &mut parser, Duration::from_millis(200)).unwrap();

    session.send("ihello world").unwrap();
    session.send("\x1b").unwrap();
    if !wait_for_text(&mut session, &mut parser, "NORMAL", Duration::from_secs(1)).unwrap() {
        session.send("\x1b").unwrap();
        wait_for_text(&mut session, &mut parser, "NORMAL", Duration::from_secs(1)).unwrap();
    }

    session.send(":w\r").unwrap();
    thread::sleep(Duration::from_millis(50));
    session.send(":q!\r").unwrap();
    session.expect(Eof).unwrap();

    let text = std::fs::read_to_string(path).expect("read file");
    assert_eq!(text, "hello world");
}

#[test]
fn render_shows_typed_text() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("screen.txt");

    let mut session = spawn_minivim(&path);
    session.set_expect_timeout(Some(Duration::from_secs(2)));

    let mut parser = Parser::new(24, 80, 0);
    drain_output(&mut session, &mut parser, Duration::from_millis(200)).unwrap();

    session.send("ihello").unwrap();
    session.send("\x1b").unwrap();
    drain_output(&mut session, &mut parser, Duration::from_millis(200)).unwrap();

    let contents = parser.screen().contents();
    session.send(":q!\r").unwrap();
    session.expect(Eof).unwrap();

    assert!(contents.contains("hello"));
}
