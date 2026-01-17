# minivim

A tiny, plugin-driven Vim-like editor in Rust. Core behavior is implemented as plugins.

## Run

```
cargo run -- path/to/file.txt
```

## Modes

- Normal: move and issue commands
- Insert: type text
- Command: run ex-style commands

## Keys

Normal mode:
- `h` `j` `k` `l` or arrows: move
- `0` / `$`: line start/end
- `i`: enter insert mode
- `:`: enter command mode
- `x`: delete char under cursor
- `Esc`: return to normal mode

Insert mode:
- Type to insert
- `Enter`: new line
- `Backspace`: delete
- `Tab`: insert four spaces
- Arrows: move
- `Esc`: return to normal mode

Command mode:
- `:w` write
- `:w filename` write to a new file
- `:q` quit (fails if dirty)
- `:q!` quit without saving
- `:wq` or `:x` write and quit

## Plugins

Core structures live in `src/editor.rs`. Basic behavior is provided by plugins in
`src/plugins.rs` (modes, motion, editing, commands, rendering, syntect-based
syntax highlighting).

## Development

- Enable the pre-commit hook: `./scripts/install-hooks.sh`
- Hook runs: `cargo clippy --all-targets -- -D warnings`
