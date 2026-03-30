# pick-ssh

fzf-like TUI for selecting SSH hosts from `~/.ssh/config`.

## Commands

```bash
cargo build          # Build
cargo run            # Run TUI (standalone mode — selects host, then exec ssh)
cargo run -- --complete  # Completion mode — prints selected hostname to stdout
cargo run -- --filter foo # Pre-fill the filter field
cargo run -- --complete --line "scp -r " # Shell integration mode (analyzes command line)
cargo run -- --embed zsh # Print shell integration snippet (zsh/bash/fish)
cargo clippy         # Lint
cargo test           # Test
cargo fmt -- --check # Format check
```

## Architecture

```
src/
├── main.rs         — CLI (clap), terminal setup, main loop, exec ssh or print host
├── ssh_config.rs   — parse ~/.ssh/config recursively, resolve Include directives
├── tree.rs         — ConfigNode / HostEntry / FlatItem data structures
├── app.rs          — App state: filter, selection, visible list, navigation, fuzzy match
├── ui.rs           — ratatui rendering: filter bar + scrollable tree list
├── event.rs        — crossterm keyboard event handling
├── history.rs      — parse ~/.zsh_history for ssh/sftp/scp/rsync hosts (with full command)
└── theme.rs        — Theme struct, 20 built-in color palettes, theme resolution
```

### Data flow

1. `ssh_config::load_ssh_config()` → `ConfigNode` tree
2. `history::load_history()` → `Vec<HistoryEntry>` (host + full command) from `~/.zsh_history`
3. `App::new(root, history)` builds combined flat list: Recent (top 10) → config nodes → History (rest)
4. `ui::draw()` renders visible items; `event::handle_events()` updates app state
5. On Enter → `--complete` prints result, otherwise execs command

### CLI flags

- `--complete` — print result to stdout instead of exec ssh
- `--filter <value>` — pre-fill filter field; auto-selects if single unique match
- `--line <buffer>` — shell integration: pass current command line for context-aware completion (supersedes `--filter`)
- `--embed <shell>` — print shell integration snippet (zsh/bash/fish) and exit
- `--theme <name>` — color theme (overrides `PICK_SSH_CONFIG` env var)

### Key design decisions

- TUI renders to **stderr** so `--complete` mode can print to stdout cleanly
- File headers (config paths) are non-selectable; arrow keys skip them
- Include directives are resolved recursively using glob patterns
- `Host *` and other wildcard patterns are skipped
- Host entries starting with `_` are filtered out
- Selected host uses SSH config entry name (alias), not resolved hostname
- Root config header shows `"config"`; child configs show parent dir name or filename
- Glob includes sorted alphabetically; explicit includes preserve declaration order

### Themes

- 20 built-in themes: gruvbox-dark (default), gruvbox-light, dracula, nord, catppuccin-mocha, catppuccin-latte, solarized-dark, solarized-light, tokyonight, tokyonight-storm, onedark, monokai, zenburn, base16-default-dark, rose-pine, rose-pine-moon, rose-pine-dawn, kanagawa, kanagawa-dragon, kanagawa-lotus
- Theme struct has 9 semantic color slots: `bg`, `bg_hl`, `fg`, `fg_dim`, `fg_bright`, `accent_filter`, `accent_marker`, `accent_header`, `accent_info`
- Resolution order: `--theme` CLI flag > `PICK_SSH_CONFIG` env var > default (gruvbox-dark)
- `PICK_SSH_CONFIG` format: comma-separated key=value pairs (e.g., `PICK_SSH_CONFIG="theme=nord"`)
- Aliases: "gruvbox" → gruvbox-dark, "catppuccin" → catppuccin-mocha, "solarized" → solarized-dark, "base16" → base16-default-dark, "rosepine" → rose-pine
- Alt+e/Alt+r cycle themes at runtime (previous/next); theme name shown in filter title while cycling

### Navigation

- Up/Down arrows navigate without modifier; Ctrl+j/k also work
- Home/End jump to first/last host
- PageUp/PageDown move by one viewport page
- Selected item always scrolls into view (ratatui ListState)

### Selection behavior

- **Tab** — select host; for history entries, uses the full original command (e.g., `ssh aaa -t zsh`)
- **Enter** — print command instead of executing: standalone prints to stdout; completion mode outputs `print:` prefixed result so shell places it on command line without executing
- In ssh-launch mode (standalone / empty line / single word), Enter on a history entry runs the full original command
- In completion mode (`scp`, `rsync`, etc.), full_cmd is not used — only the hostname is inserted

### Filtering

- Fuzzy search: pattern chars match in order but not necessarily consecutive (like fzf)
- Exact search: prefix filter with `'` to switch to exact substring match (e.g., `'prod`)
- Ctrl+F toggles between fuzzy and exact mode (adds/removes leading `'`)
- Smart case: lowercase-only query → case-insensitive; any uppercase → case-sensitive (applies to both fuzzy and exact modes)
- Parent matching: when a file header matches filter, all its children are shown
- History entries with extra args (beyond `ssh host`) also match filter against the full command
- Selection preserved on filter change: if the selected item still matches, it stays selected; otherwise the closest visible host is selected
- Auto-select: with `--filter`/`--line`, if there is a single unique match (deduplicated across Recent/config/History), it is selected immediately (TUI is skipped)

### History entries

- Full original command stored per entry (e.g., `ssh aaa -t zsh`, `rsync -avz ./dir user@server:/path`)
- Displayed in the TUI: host label on the left, full command in gray starting at column 30 (only shown when command has extra args beyond `ssh host`)
- Deduplicated by full command (so `ssh host -t zsh` and `ssh host -t bash` are distinct entries)
- Enter uses the full command in ssh-launch mode; Tab prints the command without executing

### Shell integration (`--line` / `--embed`)

- `--line ""` (empty) → output `"ssh <host>"` (or full history cmd), shell accepts and runs it
- `--line "word"` (single word, no spaces) → uses word as filter, output `"ssh <host>"` (or full history cmd), shell accepts
- `--line "cmd args "` (ends with space) → output `"cmd args <host>"`, shell sets LBUFFER
- `--line "cmd args partial"` (partial word) → uses `partial` as filter, output `"cmd args <host>"`, shell replaces partial
- `scp`/`rsync` detected as command → appends `:` after hostname
- Shell snippets are minimal: pass `--line "$LBUFFER"` to tool, set result, handle accept-line for ssh-launch cases
- `print:` prefix in output signals shell to place command on command line without executing (Tab selection)
