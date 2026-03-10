# pick-ssh

An fzf-like TUI for selecting SSH hosts from `~/.ssh/config`.

Browse, filter, and connect to your SSH hosts without remembering hostnames. pick-ssh parses your SSH config (including recursive `Include` directives) and your shell history to present a unified, searchable list.

## Features

- **SSH config parsing** — reads `~/.ssh/config` recursively, resolving `Include` directives with glob support
- **Shell history** — shows recently used SSH/SCP/SFTP/rsync hosts from `~/.zsh_history`, with full original commands
- **Fuzzy filtering** — fzf-like fuzzy matching (characters in order, not necessarily consecutive); prefix with `'` for exact substring match
- **Smart case** — lowercase-only query is case-insensitive; any uppercase character switches to case-sensitive (both fuzzy and exact)
- **Parent matching** — when a config file header matches your filter, all its hosts are shown
- **Auto-select** — if a filter yields a single unique host (deduplicated across history and config), it is selected automatically
- **History commands** — history entries display the full original command; Enter replays it, Tab prints without executing
- **Shell integration** — embed as a Ctrl+S completion widget in zsh, bash, or fish
- **Context-aware completion** — appends `:` for scp/rsync, detects partial words as filters, launches ssh on empty line

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary at target/release/pick-ssh
```

## Usage

### Standalone mode

Run `pick-ssh` to open the TUI. Select a host and press Enter to immediately SSH into it:

```bash
pick-ssh
```

### Completion mode

Print the selected hostname to stdout (for use in scripts or shell integration):

```bash
pick-ssh --complete
```

### Context-aware completion

Pass the current command line buffer for smart completion:

```bash
pick-ssh --complete --line "ssh pro"
```

The `--line` flag analyzes the command line to determine the filter, output prefix, and suffix (`:` for scp/rsync). If the line contains a single word with no spaces, pick-ssh treats it as a filter and prepends `ssh` to the result.

### Pre-filled filter

Open the TUI with the filter field pre-filled:

```bash
pick-ssh --filter "prod"
```

### Shell integration

pick-ssh can act as a TUI-powered completion helper. Press **Ctrl+S** at any point in your terminal to open the host picker. The behavior adapts to the current command line:

| Command line state | Ctrl+S behavior |
|--------------------|-----------------|
| Empty | Opens picker, then runs `ssh <host>` on Enter. Esc returns to shell. |
| Ends with space (e.g., `ssh `) | Opens picker, inserts selected host at cursor |
| Partial word (e.g., `ssh pro`) | Opens picker pre-filtered with `pro`, replaces the partial word with selected host |

Generate the shell integration snippet:

```bash
# For zsh (default)
pick-ssh --embed zsh

# For bash
pick-ssh --embed bash

# For fish
pick-ssh --embed fish
```

#### Zsh setup

Add to `~/.zshrc`:

```zsh
eval "$(pick-ssh --embed zsh)"
```

Or manually paste the snippet from `pick-ssh --embed zsh`.

#### Bash setup

Add to `~/.bashrc`:

```bash
eval "$(pick-ssh --embed bash)"
```

#### Fish setup

Add to `~/.config/fish/config.fish`:

```fish
pick-ssh --embed fish | source
```

After reloading your shell, press **Ctrl+S** to open pick-ssh. The widget is context-aware — on an empty line it acts as a quick SSH launcher, otherwise it completes hostnames inline for any command (`ssh`, `scp`, `rsync`, etc.).

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| Type any character | Filter hosts (fuzzy match) |
| `'` prefix | Switch to exact substring match (e.g., `'prod`) |
| Ctrl+F | Toggle fuzzy/exact mode (adds/removes `'` prefix) |
| Backspace | Delete last filter character |
| Up / Ctrl+K | Move selection up (skips headers) |
| Down / Ctrl+J | Move selection down (skips headers) |
| Home | Jump to first host |
| End | Jump to last host |
| PageUp | Move up one page |
| PageDown | Move down one page |
| Enter | Select host — for history entries, uses the full original command |
| Tab | Print command without executing (standalone: prints to stdout; shell widget: places on command line) |
| Esc / Ctrl+C | Quit without selecting |

## How it works

1. Parses `~/.ssh/config` recursively (honoring `Include` directives)
2. Reads `~/.zsh_history` for recently used ssh, sftp, scp, and rsync hosts (deduplicated, most recent first)
3. Displays a combined list: **Recent** (top 10 from history) → **config hosts** → **History** (remaining)
4. History entries show the full original command at column 30 (only when command has extra args beyond `ssh host`)
5. History is deduplicated by full command, so `ssh host -t zsh` and `ssh host -t bash` are distinct entries
6. TUI renders to stderr so `--complete` mode can write cleanly to stdout
7. Filtering is fuzzy (fzf-style) or exact (`'` prefix), with smart case and selection preservation
8. History entries with extra args are also matched against the filter
9. If a filter yields exactly one unique host (deduplicated across all sections), it is auto-selected without showing the TUI
10. On Enter: standalone mode runs the full history command (or `exec ssh <host>`); completion mode prints the hostname
11. On Tab: prints the command without executing — in shell widget mode, places it on the command line for editing
12. For scp/rsync commands (detected via `--line`), `:` is appended after the hostname

## SSH config support

- `Host` entries (wildcard patterns like `Host *` and entries starting with `_` are skipped)
- `Include` directives with glob patterns (e.g., `Include config.d/*`)
- `Hostname`, `User`, `Port` fields are parsed (the config alias name is used for connection)
- Nested includes are resolved recursively
- Glob includes are sorted alphabetically; explicit includes preserve order

## License

MIT
