//! PTY-based integration tests. Spawns the binary in an invisible pseudo-terminal,
//! sends keystrokes, parses the rendered screen with vt100, and asserts on the grid.
//!
//! These tests exist so we never have to ask the user "what do you see?" — the test
//! drives the actual binary and reads what it draws.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use tempfile::TempDir;

const ROWS: u16 = 30;
const COLS: u16 = 100;

/// Builder for a fake $HOME directory + spawned pick-ssh in a PTY.
struct HarnessBuilder {
    files: HashMap<PathBuf, String>,
    /// Files that should be marked executable (chmod 755).
    executables: Vec<PathBuf>,
    env: HashMap<String, String>,
}

impl HarnessBuilder {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
            executables: Vec::new(),
            env: HashMap::new(),
        }
    }

    /// Add a file at `relative_path` (under fake $HOME) with the given content.
    fn file(mut self, relative_path: &str, content: impl Into<String>) -> Self {
        self.files.insert(PathBuf::from(relative_path), content.into());
        self
    }

    /// Mark a previously-added file as executable.
    #[allow(dead_code)]
    fn executable(mut self, relative_path: &str) -> Self {
        self.executables.push(PathBuf::from(relative_path));
        self
    }

    /// Set an environment variable for the spawned binary.
    fn env(mut self, key: &str, value: impl Into<String>) -> Self {
        self.env.insert(key.to_string(), value.into());
        self
    }

    fn spawn(self) -> Tui {
        use std::os::unix::fs::PermissionsExt;

        let home = TempDir::new().expect("tempdir");
        for (rel, content) in &self.files {
            let path = home.path().join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir parent");
            }
            fs::write(&path, content).expect("write fixture");
        }
        for rel in &self.executables {
            let path = home.path().join(rel);
            let mut perms = fs::metadata(&path).expect("stat fixture").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).expect("chmod fixture");
        }

        let pty = NativePtySystem::default();
        let pair = pty
            .openpty(PtySize {
                rows: ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");

        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_pick-ssh"));
        cmd.env("HOME", home.path());
        cmd.env("TERM", "xterm-256color");
        // Pin a theme so renders are deterministic.
        cmd.env("PICK_SSH_CONFIG", "theme=gruvbox-dark");
        for (k, v) in &self.env {
            cmd.env(k, v);
        }

        let child = pair.slave.spawn_command(cmd).expect("spawn");
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().expect("clone reader");
        let writer = pair.master.take_writer().expect("take writer");
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });

        let parser = vt100::Parser::new(ROWS, COLS, 0);
        let mut h = Tui {
            _home: home,
            parser,
            writer: Some(writer),
            rx,
            child: Some(child),
            master: Some(pair.master),
        };
        h.settle();
        h
    }
}

struct Tui {
    _home: TempDir,
    parser: vt100::Parser,
    writer: Option<Box<dyn Write + Send>>,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
}

impl Tui {
    /// Wait until the screen has been quiet for `quiet` ms, up to a max.
    fn settle(&mut self) {
        let max = Duration::from_millis(800);
        let quiet = Duration::from_millis(80);
        let deadline = Instant::now() + max;
        let mut last_recv = Instant::now();
        while Instant::now() < deadline {
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(bytes) => {
                    self.parser.process(&bytes);
                    last_recv = Instant::now();
                }
                Err(_) => {
                    if last_recv.elapsed() >= quiet {
                        return;
                    }
                }
            }
        }
    }

    fn send(&mut self, s: &str) {
        let w = self.writer.as_mut().expect("writer dropped");
        w.write_all(s.as_bytes()).expect("write keys");
        w.flush().ok();
    }

    /// Returns the rendered screen as a Vec<String>, one per row, right-trimmed.
    fn screen(&self) -> Vec<String> {
        let screen = self.parser.screen();
        (0..ROWS)
            .map(|r| {
                let mut s = String::new();
                for c in 0..COLS {
                    if let Some(cell) = screen.cell(r, c) {
                        s.push_str(&cell.contents());
                    }
                }
                s.trim_end().to_string()
            })
            .collect()
    }

    /// Index of the first row containing the substring.
    fn row_of(&self, needle: &str) -> Option<usize> {
        self.screen().into_iter().position(|row| row.contains(needle))
    }

    /// Send Esc to ask the TUI to quit. Cleanup happens in Drop.
    fn quit(&mut self) {
        if let Some(w) = self.writer.as_mut() {
            let _ = w.write_all(&[0x1b]);
            let _ = w.flush();
        }
    }

    fn home_path(&self) -> &std::path::Path {
        self._home.path()
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        // Kill the child first so it stops reading from the PTY, then drop master.
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = self.master.take();
        let _ = self.writer.take();
    }
}

#[test]
fn ctrl_e_works_when_pickssh_stdout_is_captured() {
    // Reproduces the shell-integration bug: in zsh's `result=$(pick-ssh)` the binary's
    // stdout is a pipe, not the terminal. Without the /dev/tty fix the editor's output
    // would go into the pipe and the user sees nothing happen.
    use std::os::unix::fs::PermissionsExt;

    let editor_dir = TempDir::new().expect("editor tempdir");
    let editor_path = editor_dir.path().join("fake-editor");
    // Editor logs its args AND prints a marker to stdout. If the marker ends up in
    // pick-ssh's captured stdout file, the editor's stdio was incorrectly inherited
    // from pick-ssh instead of routed to /dev/tty.
    fs::write(
        &editor_path,
        "#!/bin/sh\necho \"$@\" > \"$HOME/editor.log\"\necho EDITOR_STDOUT_MARKER\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&editor_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&editor_path, perms).unwrap();

    // Use a wrapper shell to redirect stdout to a file before exec-ing pick-ssh.
    let home = TempDir::new().expect("home");
    let ssh_dir = home.path().join(".ssh");
    fs::create_dir_all(&ssh_dir).unwrap();
    fs::write(
        ssh_dir.join("config"),
        "Host alpha\n  HostName 10.0.0.1\n",
    )
    .unwrap();

    let captured_path = home.path().join("captured.out");
    let bin = env!("CARGO_BIN_EXE_pick-ssh").to_string();
    let cmd_line = format!(
        "exec {} > {}",
        shell_escape(&bin),
        shell_escape(&captured_path.display().to_string())
    );

    let pty = NativePtySystem::default();
    let pair = pty
        .openpty(PtySize {
            rows: ROWS,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut cmd = CommandBuilder::new("sh");
    cmd.arg("-c");
    cmd.arg(&cmd_line);
    cmd.env("HOME", home.path());
    cmd.env("TERM", "xterm-256color");
    cmd.env("PICK_SSH_CONFIG", "theme=gruvbox-dark");
    cmd.env("EDITOR", editor_path.display().to_string());
    let mut child = pair.slave.spawn_command(cmd).unwrap();
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    // Settle, send Ctrl+E, settle.
    let settle = |rx: &std::sync::mpsc::Receiver<Vec<u8>>| {
        let max = std::time::Duration::from_millis(800);
        let quiet = std::time::Duration::from_millis(80);
        let deadline = std::time::Instant::now() + max;
        let mut last = std::time::Instant::now();
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(std::time::Duration::from_millis(20)) {
                Ok(_) => last = std::time::Instant::now(),
                Err(_) => {
                    if last.elapsed() >= quiet {
                        return;
                    }
                }
            }
        }
    };
    settle(&rx);
    writer.write_all(&[0x05]).unwrap(); // Ctrl+E
    writer.flush().ok();
    settle(&rx);

    // Ask the binary to quit so the test exits cleanly.
    writer.write_all(&[0x1b]).ok();
    writer.flush().ok();
    drop(writer);
    drop(pair.master);
    let _ = child.kill();
    let _ = child.wait();

    let log_path = home.path().join("editor.log");
    let log_content = fs::read_to_string(&log_path).unwrap_or_else(|e| {
        panic!("editor log missing at {} — editor never invoked: {}", log_path.display(), e)
    });
    assert!(
        log_content.contains("/.ssh/config"),
        "editor log should contain config path; got: {log_content}"
    );

    // The marker must NOT end up in the captured stdout file: that would mean the
    // editor inherited pick-ssh's redirected stdout instead of using /dev/tty.
    let captured = fs::read_to_string(&captured_path).unwrap_or_default();
    assert!(
        !captured.contains("EDITOR_STDOUT_MARKER"),
        "editor stdout leaked into pick-ssh's captured stdout. captured:\n{captured}"
    );
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[test]
fn ctrl_e_invokes_editor_with_correct_file_and_line() {
    // Fake $EDITOR is a shell script that writes its args to $HOME/editor.log.
    // The script lives in an external tempdir so its path is known before spawning
    // the TUI. After Ctrl+E we verify the log contains the expected file path and +line.
    use std::os::unix::fs::PermissionsExt;

    let editor_dir = TempDir::new().expect("editor tempdir");
    let editor_path = editor_dir.path().join("fake-editor");
    fs::write(
        &editor_path,
        "#!/bin/sh\necho \"$@\" > \"$HOME/editor.log\"\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&editor_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&editor_path, perms).unwrap();

    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            // Host alpha on line 1, Host beta on line 4.
            "Host alpha\n  HostName 10.0.0.1\n\nHost beta\n  HostName 10.0.0.2\n",
        )
        .env("EDITOR", editor_path.display().to_string())
        .spawn();

    let home = tui.home_path().to_owned();

    // Move down to the second host (beta, on line 4 of the config file), then Ctrl+E.
    tui.send("\x1b[B"); // arrow down
    tui.settle();
    tui.send("\x05"); // Ctrl+E
    tui.settle();

    let log_path = home.join("editor.log");
    let log_content = fs::read_to_string(&log_path)
        .unwrap_or_else(|e| panic!("editor log missing at {}: {}", log_path.display(), e));

    assert!(
        log_content.contains("+4"),
        "expected +4 (line of `Host beta`) in editor args, got: {log_content}"
    );
    assert!(
        log_content.contains("/.ssh/config"),
        "expected config file path in editor args, got: {log_content}"
    );

    tui.quit();
}

#[test]
fn ctrl_e_on_known_host_shows_status_message() {
    // Known-hosts entries have no source file; pressing Ctrl+E should display the
    // "Edit only available for config entries" status without launching anything.
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "")
        .file(
            ".ssh/known_hosts",
            "10.0.0.99 ssh-rsa AAAA\n",
        )
        .env("EDITOR", "/bin/false")
        .spawn();

    // Selection lands on the first host (10.0.0.99 in Known section).
    tui.send("\x05"); // Ctrl+E
    tui.settle();

    let screen = tui.screen();
    let joined = screen.join("\n");
    assert!(
        joined.contains("Edit only available"),
        "status message missing.\n{joined}"
    );

    tui.quit();
}

#[test]
fn known_hosts_appear_in_their_own_section() {
    // ~/.ssh/known_hosts has a mix of plain entries, comma-separated aliases,
    // bracketed-port form, hashed (skipped), and a comment. Only the plain ones
    // should appear under a "Known" section.
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host alpha\n  HostName 10.0.0.1\n")
        .file(
            ".ssh/known_hosts",
            "\
# auto-managed
plain.example.com ssh-rsa AAAA
multi.example.com,1.2.3.4 ssh-ed25519 AAAA
[ported.example.com]:2222 ssh-ed25519 AAAA
|1|hashed=|hashed= ssh-rsa AAAA
",
        )
        .spawn();

    let screen = tui.screen();
    let known_hdr = screen
        .iter()
        .position(|row| row.contains("Known"))
        .unwrap_or_else(|| panic!("Known section header missing.\n{}", screen.join("\n")));

    let joined = screen.join("\n");
    assert!(
        joined.contains("plain.example.com"),
        "plain entry missing.\n{joined}"
    );
    assert!(
        joined.contains("multi.example.com"),
        "comma-aliased entry missing.\n{joined}"
    );
    assert!(
        joined.contains("1.2.3.4"),
        "comma-aliased ip missing.\n{joined}"
    );
    assert!(
        joined.contains("ported.example.com"),
        "[host]:port form missing.\n{joined}"
    );
    assert!(
        !joined.contains("hashed"),
        "hashed entry should not appear.\n{joined}"
    );
    // Section header should come before any of its entries.
    let plain_row = screen
        .iter()
        .position(|row| row.contains("plain.example.com"))
        .unwrap();
    assert!(known_hdr < plain_row);

    tui.quit();
}

#[test]
fn known_host_dedupes_against_config_alias() {
    // If `~/.ssh/config` has an alias whose name matches a known_hosts entry,
    // the known_hosts entry should not show up duplicated.
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host shared.example.com\n  HostName 10.0.0.1\n")
        .file(
            ".ssh/known_hosts",
            "shared.example.com ssh-rsa AAAA\nunique.example.com ssh-rsa AAAA\n",
        )
        .spawn();

    let screen = tui.screen();
    let joined = screen.join("\n");

    // `shared.example.com` should appear exactly once (under config), and `unique`
    // should appear once under Known.
    let shared_count = joined.matches("shared.example.com").count();
    let unique_count = joined.matches("unique.example.com").count();
    assert_eq!(
        shared_count, 1,
        "shared host should appear once, not duplicated.\n{joined}"
    );
    assert_eq!(unique_count, 1, "unique host missing.\n{joined}");

    tui.quit();
}

#[test]
fn add_host_form_saves_to_existing_file() {
    // Open form with Ctrl+N, type alias and hostname, confirm. The new alias should
    // be appended to ~/.ssh/config.
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host already-here\n  HostName 10.0.0.99\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();

    tui.send("\x0e"); // Ctrl+N
    tui.settle();

    // Field 0: Alias
    tui.send("newhost");
    // Tab to HostName
    tui.send("\t");
    tui.send("10.0.0.42");
    // Submit
    tui.send("\r"); // Enter
    tui.settle();

    let cfg = fs::read_to_string(home.join(".ssh/config")).unwrap();
    assert!(
        cfg.contains("Host newhost"),
        "config should contain new alias. content:\n{cfg}"
    );
    assert!(
        cfg.contains("HostName 10.0.0.42"),
        "config should contain new hostname. content:\n{cfg}"
    );
    // Old entry preserved.
    assert!(cfg.contains("Host already-here"));

    // Exactly one blank line between the previous entry and the new one (no
    // multiple blanks, no missing separator).
    assert!(
        cfg.contains("HostName 10.0.0.99\n\nHost newhost"),
        "expected exactly one blank line before new block:\n{cfg:?}"
    );

    tui.quit();
}

#[test]
fn add_host_form_rejects_empty_alias() {
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host placeholder\n  HostName 10.0.0.1\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();

    tui.send("\x0e"); // Ctrl+N
    tui.settle();
    // Don't type anything, just press Enter
    tui.send("\r");
    tui.settle();

    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("Alias is required"),
        "expected validation error, got:\n{screen}"
    );

    let cfg = fs::read_to_string(home.join(".ssh/config")).unwrap();
    assert!(
        !cfg.contains("Host \n"),
        "no empty Host should have been written"
    );

    tui.quit();
}

#[test]
fn add_host_form_rejects_duplicate_alias() {
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host existing\n  HostName 10.0.0.1\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    tui.send("\x0e"); // Ctrl+N
    tui.settle();
    tui.send("existing");
    tui.send("\r");
    tui.settle();

    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("already exists"),
        "expected duplicate-alias error, got:\n{screen}"
    );

    tui.quit();
}

#[test]
fn ctrl_t_inside_add_host_form_shows_test_on_top() {
    // Open the add-host form, type a hostname, press Ctrl+T. The SSH test popup
    // (titled "Testing …") should appear above the form. Pressing Esc should
    // dismiss the test and leave the form still active.
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host placeholder\n  HostName 10.0.0.1\n")
        // Make ssh resolve quickly to a closed port (no DNS lookup). We just
        // need the test popup to appear; the actual outcome is irrelevant.
        .env("EDITOR", "/bin/false")
        .spawn();

    tui.send("\x0e"); // Ctrl+N opens the form
    tui.settle();
    tui.send("probehost");
    // Tab to HostName
    tui.send("\t");
    tui.send("127.0.0.1");
    tui.send("\x14"); // Ctrl+T
    tui.settle();

    let with_test = tui.screen().join("\n");
    assert!(
        with_test.contains("Testing 127.0.0.1") || with_test.contains("- Failed") || with_test.contains("- Success"),
        "test popup should appear with title.\n{with_test}"
    );

    // Dismiss the test popup with Esc.
    tui.send("\x1b");
    tui.settle();

    let after_dismiss = tui.screen().join("\n");
    assert!(
        after_dismiss.contains("Add new SSH host") || after_dismiss.contains("Alias"),
        "form should still be visible after closing the test popup.\n{after_dismiss}"
    );
    // Test popup gone.
    assert!(
        !after_dismiss.contains("Testing 127.0.0.1"),
        "test popup should have been dismissed.\n{after_dismiss}"
    );

    tui.quit();
}

#[test]
fn ctrl_d_duplicates_selected_host_with_inserted_block() {
    // Two hosts in the file. Selecting the first and pressing Ctrl+D opens the form
    // pre-filled with `alpha-copy` + alpha's fields. Saving should insert the new
    // block immediately after alpha (between alpha and beta), not at the end.
    let cfg_initial = "\
Host alpha
  HostName 10.0.0.1
  User admin
  Port 2222

Host beta
  HostName 10.0.0.2
";
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", cfg_initial)
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();

    // Cursor lands on alpha (first host).
    tui.send("\x04"); // Ctrl+D
    tui.settle();

    // Form is pre-filled. The screen should show "alpha-copy" as the alias and
    // alpha's fields populated.
    let screen_before = tui.screen().join("\n");
    assert!(
        screen_before.contains("alpha-copy"),
        "alias should default to 'alpha-copy'.\n{screen_before}"
    );
    assert!(
        screen_before.contains("10.0.0.1"),
        "HostName should be pre-filled.\n{screen_before}"
    );
    assert!(
        screen_before.contains("admin"),
        "User should be pre-filled.\n{screen_before}"
    );
    assert!(
        screen_before.contains("2222"),
        "Port should be pre-filled.\n{screen_before}"
    );

    // Save with default suggested alias (no conflict since '-copy' suffix is unique).
    tui.send("\r");
    tui.settle();

    let cfg = fs::read_to_string(home.join(".ssh/config")).unwrap();
    let alpha_pos = cfg.find("Host alpha\n").expect("alpha block present");
    let copy_pos = cfg.find("Host alpha-copy").expect("copy block present");
    let beta_pos = cfg.find("Host beta\n").expect("beta block present");
    assert!(
        alpha_pos < copy_pos && copy_pos < beta_pos,
        "alpha-copy must sit between alpha and beta.\nfile contents:\n{cfg}"
    );
    // The duplicate must carry over the user/port.
    let after_copy = &cfg[copy_pos..];
    let next_host = after_copy
        .find("Host beta")
        .expect("beta should follow alpha-copy");
    let copy_block = &after_copy[..next_host];
    assert!(
        copy_block.contains("HostName 10.0.0.1"),
        "copy block missing HostName:\n{copy_block}"
    );
    assert!(
        copy_block.contains("User admin"),
        "copy block missing User:\n{copy_block}"
    );
    assert!(
        copy_block.contains("Port 2222"),
        "copy block missing Port:\n{copy_block}"
    );

    // The duplicate must be sandwiched by exactly one blank line on each side.
    assert!(
        cfg.contains("\n\nHost alpha-copy"),
        "expected blank line before duplicate:\n{cfg:?}"
    );
    // The block ends with the last field's newline; right after, a blank line
    // and then the next block.
    assert!(
        cfg.contains("Port 2222\n\nHost beta") || cfg.contains("Port 2222\n  \n\nHost beta"),
        "expected blank line after duplicate before next block. file:\n{cfg:?}"
    );

    tui.quit();
}

/// Returns the line of `content` that comes immediately after `marker_line`,
/// where `marker_line` is the LAST line of the inserted block (assumed to be a
/// "  Field value" config line or empty). Skips trailing blank lines that the
/// writer added between the new block and the next thing.
fn line_after_block(content: &str, last_block_line: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let pos = lines.iter().position(|l| *l == last_block_line)?;
    // Skip blank lines after the block.
    let mut i = pos + 1;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    Some(lines.get(i).unwrap_or(&"").to_string())
}

/// Word-boundary directive check: matches `Host` but NOT `HostName`,
/// `Match` but NOT `MatchUser`, etc.
fn is_directive(line: &str, name: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.len() < name.len() {
        return false;
    }
    let (head, rest) = trimmed.split_at(name.len());
    if !head.eq_ignore_ascii_case(name) {
        return false;
    }
    matches!(rest.chars().next(), None | Some(' ' | '\t' | '='))
}

fn is_host_or_eof(line: &str) -> bool {
    line.is_empty()
        || is_directive(line, "Host")
        || is_directive(line, "Match")
        || is_directive(line, "Include")
}

#[test]
fn add_host_insertion_lands_before_host_line_or_eof_for_ctrl_n() {
    // Ctrl+N appends at EOF — the line after the inserted block must be empty.
    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            "Host alpha\n  HostName 1.1.1.1\n\nHost beta\n  HostName 2.2.2.2\n",
        )
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();
    tui.send("\x0e"); // Ctrl+N
    tui.settle();
    tui.send("appended");
    tui.send("\t"); // → HostName
    tui.send("9.9.9.9");
    tui.send("\r");
    tui.settle();

    let cfg = fs::read_to_string(home.join(".ssh/config")).unwrap();
    let after = line_after_block(&cfg, "  HostName 9.9.9.9").expect("inserted line found");
    assert!(
        is_host_or_eof(&after),
        "after the new block expected EOF or a Host line, got: {after:?}\nfull file:\n{cfg}"
    );
    tui.quit();
}

#[test]
fn add_host_insertion_lands_before_host_line_or_eof_for_ctrl_d_middle() {
    // Ctrl+D on a host in the middle of the file: the new block must land right
    // before the NEXT Host directive (after blank-line separator).
    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            "Host alpha\n  HostName 1.1.1.1\n\nHost beta\n  HostName 2.2.2.2\n\nHost gamma\n  HostName 3.3.3.3\n",
        )
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();
    // Cursor lands on first host (alpha).
    tui.send("\x04"); // Ctrl+D — duplicates alpha
    tui.settle();
    tui.send("\r"); // accept default alias `alpha-copy`
    tui.settle();

    let cfg = fs::read_to_string(home.join(".ssh/config")).unwrap();
    // The duplicate's last property is "  HostName 1.1.1.1" (same as alpha).
    // There are now TWO of them in the file; the second occurrence is the copy.
    let mut parts = cfg.match_indices("  HostName 1.1.1.1");
    let _first = parts.next().expect("alpha's HostName");
    let (second_pos, _) = parts.next().expect("alpha-copy's HostName");
    let mut iter = cfg[second_pos..].lines().skip(1);
    let after = loop {
        match iter.next() {
            Some(l) if l.trim().is_empty() => continue,
            Some(l) => break l.to_string(),
            None => break String::new(),
        }
    };
    assert!(
        is_host_or_eof(&after),
        "after duplicate expected Host line, got: {after:?}\nfull file:\n{cfg}"
    );
    assert!(
        after.starts_with("Host beta"),
        "duplicate of alpha must land right before beta, got: {after:?}\nfull file:\n{cfg}"
    );
    tui.quit();
}

#[test]
fn ctrl_p_edits_properties_in_place() {
    // Open Properties (Ctrl+P) on a config entry, change one field, save.
    // The block should be replaced in-place: alias kept, HostName updated, no duplicate.
    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            "Host alpha\n  HostName 10.0.0.1\n  User old\n\nHost beta\n  HostName 10.0.0.2\n",
        )
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();

    // Cursor on alpha. Open properties.
    tui.send("\x10"); // Ctrl+P
    tui.settle();
    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("Edit SSH host"),
        "form title should say Edit. screen:\n{screen}"
    );
    assert!(
        screen.contains("alpha") && screen.contains("10.0.0.1") && screen.contains("old"),
        "fields should be pre-filled. screen:\n{screen}"
    );

    // Tab x2 from Alias → HostName → User, clear field, type new value.
    tui.send("\t\t");
    tui.send("\x15"); // Ctrl+U: clear field
    tui.send("newuser");
    tui.send("\r");
    tui.settle();

    let cfg = fs::read_to_string(home.join(".ssh/config")).unwrap();
    // alpha block should still exist exactly once, with the updated user.
    assert_eq!(cfg.matches("Host alpha\n").count(), 1, "single alpha block. file:\n{cfg}");
    assert!(
        cfg.contains("Host alpha\n  HostName 10.0.0.1\n  User newuser\n"),
        "alpha block should have updated user. file:\n{cfg}"
    );
    // beta block must remain intact.
    assert!(
        cfg.contains("Host beta\n  HostName 10.0.0.2\n"),
        "beta block must remain. file:\n{cfg}"
    );

    tui.quit();
}

#[test]
fn ctrl_p_keeping_same_alias_does_not_trigger_conflict() {
    // Editing an entry while keeping the alias the same must NOT report a duplicate
    // alias error (the entry being edited is excluded from the conflict check).
    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            "Host alpha\n  HostName 10.0.0.1\n",
        )
        .env("EDITOR", "/bin/false")
        .spawn();

    tui.send("\x10"); // Ctrl+P
    tui.settle();
    // Press Enter without changing anything. Should save successfully.
    tui.send("\r");
    tui.settle();

    let screen = tui.screen().join("\n");
    assert!(
        !screen.contains("already exists"),
        "should not warn about self-conflict. screen:\n{screen}"
    );
    tui.quit();
}

#[test]
fn ctrl_m_moves_entry_between_files() {
    // Move alpha from ~/.ssh/config to an Include'd file. After save:
    // alpha should be GONE from ~/.ssh/config and PRESENT in the target file.
    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            "Include ~/.ssh/extra\n\nHost alpha\n  HostName 10.0.0.1\n\nHost beta\n  HostName 10.0.0.2\n",
        )
        .file(".ssh/extra", "")
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();

    // Filter to alpha so we know we're selecting it.
    tui.send("alpha");
    tui.settle();
    tui.send("\x1bm"); // Alt+M (ESC + m)
    tui.settle();
    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("Move SSH host to file"),
        "title should say Move. screen:\n{screen}"
    );

    // The File field is focused; clear it and type the target path.
    tui.send("\x15"); // Ctrl+U
    tui.send("~/.ssh/extra");
    tui.send("\r");
    tui.settle();

    let main = fs::read_to_string(home.join(".ssh/config")).unwrap();
    let extra = fs::read_to_string(home.join(".ssh/extra")).unwrap();
    assert!(
        !main.contains("Host alpha"),
        "alpha should be removed from main config. file:\n{main}"
    );
    assert!(
        main.contains("Host beta"),
        "beta should remain in main config. file:\n{main}"
    );
    assert!(
        extra.contains("Host alpha\n  HostName 10.0.0.1"),
        "alpha should be in the extra file. file:\n{extra}"
    );

    tui.quit();
}

#[test]
fn ctrl_m_on_known_host_shows_status_message() {
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "")
        .file(".ssh/known_hosts", "10.0.0.99 ssh-rsa AAAA\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    tui.send("\x1bm"); // Alt+M (ESC + m)
    tui.settle();

    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("Move only available"),
        "expected refusal status. screen:\n{screen}"
    );
    tui.quit();
}

#[test]
fn alt_b_toggles_pin_on_selected_host() {
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host alpha\n  HostName 10.0.0.1\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    let screen0 = tui.screen().join("\n");
    assert!(
        !screen0.contains("Pinned"),
        "no Pinned section initially. screen:\n{screen0}"
    );

    // Alt+B (ESC + b) to pin alpha. Alt is used (not Ctrl) so the binding
    // doesn't collide with the tmux prefix Ctrl+B.
    tui.send("\x1bb");
    tui.settle();
    let screen1 = tui.screen().join("\n");
    assert!(
        screen1.contains("Pinned"),
        "Pinned section should appear after Alt+B. screen:\n{screen1}"
    );

    // Alt+B again to unpin.
    tui.send("\x1bb");
    tui.settle();
    let screen2 = tui.screen().join("\n");
    assert!(
        !screen2.contains("Pinned"),
        "Pinned section should disappear after second Alt+B. screen:\n{screen2}"
    );

    tui.quit();
}

#[test]
fn alt_t_cycles_theme_forward() {
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host alpha\n  HostName 10.0.0.1\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    // Default theme is gruvbox-dark per the harness env. After Alt+T, the title
    // changes to show the new theme name (next in the THEMES list).
    tui.send("\x1bt"); // Alt+T (ESC + t)
    tui.settle();
    let lines = tui.screen();
    let screen = lines.join("\n");
    // The filter title now shows the theme name. We don't assert a specific
    // name (theme order can change) — just that it's NOT "Filter" anymore.
    let filter_row = lines
        .iter()
        .find(|line| line.contains('┌'))
        .cloned()
        .unwrap_or_default();
    assert!(
        !filter_row.contains("Filter"),
        "after Alt+T the filter title should show a theme name, not 'Filter'. row:\n{filter_row}\nscreen:\n{screen}"
    );

    tui.quit();
}

#[test]
fn ctrl_p_on_known_host_shows_status_message() {
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "")
        .file(".ssh/known_hosts", "10.0.0.99 ssh-rsa AAAA\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    tui.send("\x10"); // Ctrl+P
    tui.settle();

    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("Properties only available"),
        "expected refusal status message. screen:\n{screen}"
    );
    tui.quit();
}

#[test]
fn ctrl_d_to_different_file_does_not_split_existing_block() {
    // Reproduces the user's scenario: duplicate homelab (in ~/.ssh/config),
    // change File to ~/.ssh/extra (an Include'd file). The source's end_line was
    // computed against ~/.ssh/config and points mid-block in the extra file. The
    // writer must advance the insertion point past extra's properties so the new
    // block lands BEFORE a Host directive (or at EOF), never splitting extraB1.
    let main_cfg = "\
Include ~/.ssh/extra

Host homelab
  HostName 192.168.88.200
  User root
";
    // Important: the FIRST Host directive in the extra file lives at the same
    // line number where homelab's end is in the main config (line 5). That
    // way the buggy behavior (using insert_after_line as-is) would drop the new
    // block right after `Host extraB1` and before `HostName 10.9.1.11`.
    let extra_cfg = "\
# extra lab
# managed cluster

Host extraB1
    HostName 10.9.1.11

Host extraB2
    HostName 10.9.1.43
";
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", main_cfg)
        .file(".ssh/extra", extra_cfg)
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();

    // Filter to find homelab (it's in main config).
    tui.send("homelab");
    tui.settle();
    // Ctrl+D opens the duplicate form pre-filled with homelab's fields.
    tui.send("\x04");
    tui.settle();
    // Tab to the File field (6 tabs from Alias).
    for _ in 0..6 {
        tui.send("\t");
    }
    // Clear the file path and type the extra path.
    tui.send("\x15"); // Ctrl+U
    tui.send("~/.ssh/extra");
    tui.settle();
    tui.send("\r");
    tui.settle();

    let extra_after = fs::read_to_string(home.join(".ssh/extra")).unwrap();

    // extraB1's block must remain intact — `HostName 10.9.1.11` directly follows
    // `Host extraB1` with no foreign content between them.
    assert!(
        extra_after.contains("Host extraB1\n    HostName 10.9.1.11"),
        "extraB1 block was split. file:\n{extra_after}"
    );
    // The new block sits between extraB1 and extraB2, with blank-line separators.
    assert!(
        extra_after.contains("HostName 10.9.1.11\n\nHost homelab-copy"),
        "new block should land right after extraB1's full block. file:\n{extra_after}"
    );
    assert!(
        extra_after.contains("User root\n\nHost extraB2"),
        "next existing block (extraB2) should follow with one blank line. file:\n{extra_after}"
    );

    tui.quit();
}

#[test]
fn add_host_insertion_lands_at_eof_for_ctrl_d_on_last_host() {
    // Ctrl+D on the LAST host in the file: insertion lands at EOF.
    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            "Host alpha\n  HostName 1.1.1.1\n\nHost beta\n  HostName 2.2.2.2\n",
        )
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();
    // Move down to second host (beta).
    tui.send("\x1b[B");
    tui.settle();
    tui.send("\x04"); // Ctrl+D
    tui.settle();
    tui.send("\r"); // accept default alias `beta-copy`
    tui.settle();

    let cfg = fs::read_to_string(home.join(".ssh/config")).unwrap();
    // The duplicate of beta is at the end of the file. Find the LAST occurrence
    // of "  HostName 2.2.2.2" — that's the duplicate's last property line.
    let last_pos = cfg.rfind("  HostName 2.2.2.2").unwrap();
    let mut iter = cfg[last_pos..].lines().skip(1);
    let after_last = loop {
        match iter.next() {
            Some(l) if l.trim().is_empty() => continue,
            Some(l) => break l.to_string(),
            None => break String::new(),
        }
    };
    assert!(
        is_host_or_eof(&after_last),
        "after duplicate of last host expected EOF or Host, got: {after_last:?}\nfull file:\n{cfg}"
    );
    assert!(
        after_last.is_empty(),
        "after duplicate of last host expected EOF, got: {after_last:?}\nfull file:\n{cfg}"
    );
    tui.quit();
}

#[test]
fn ctrl_d_on_known_host_opens_form_with_prefilled_hostname() {
    // For non-config items (Known/Recent/History/Pinned), Ctrl+D opens the same
    // add-host form with whatever data we can derive (e.g. HostName from the
    // known_hosts entry), defaulting File to ~/.ssh/config in append mode.
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host placeholder\n  HostName 10.0.0.1\n")
        .file(".ssh/known_hosts", "10.0.0.99 ssh-rsa AAAA\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    // Filter to land on the known_hosts entry.
    tui.send("10.0.0.99");
    tui.settle();
    tui.send("\x04"); // Ctrl+D
    tui.settle();

    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("Add new SSH host") || screen.contains("Alias"),
        "form should be open.\n{screen}"
    );
    assert!(
        screen.contains("10.0.0.99"),
        "HostName should be pre-filled from the known_hosts entry.\n{screen}"
    );
    assert!(
        screen.contains(".ssh/config"),
        "File should default to ~/.ssh/config.\n{screen}"
    );

    tui.quit();
}

#[test]
fn add_host_form_creates_new_file_after_confirmation() {
    // File field points at a non-existent path. Enter triggers confirm; pressing y
    // creates the file (with parent dirs) and writes the entry.
    let mut tui = HarnessBuilder::new()
        .file(".ssh/config", "Host placeholder\n  HostName 10.0.0.1\n")
        .env("EDITOR", "/bin/false")
        .spawn();

    let home = tui.home_path().to_owned();

    tui.send("\x0e"); // Ctrl+N
    tui.settle();
    tui.send("brandnew");
    // Tab x6 to reach File field
    for _ in 0..6 {
        tui.send("\t");
    }
    // Clear the default and type a new path
    tui.send("\x15"); // Ctrl+U: clear field
    tui.send("~/.ssh/extra/work_hosts");
    tui.send("\r");
    tui.settle();

    let screen = tui.screen().join("\n");
    assert!(
        screen.contains("Create file") && screen.contains("y"),
        "expected create-file confirmation, got:\n{screen}"
    );

    // Confirm
    tui.send("y");
    tui.settle();

    let target = home.join(".ssh/extra/work_hosts");
    let cfg = fs::read_to_string(&target)
        .unwrap_or_else(|e| panic!("file not created at {}: {}", target.display(), e));
    assert!(
        cfg.contains("Host brandnew"),
        "new file should contain the host. content:\n{cfg}"
    );

    tui.quit();
}

#[test]
fn ueransim_in_subsection_floats_to_top_when_filtering_ue() {
    // Reproduces the user's case: `work` Include sits inside the main config; it has
    // a `ueransim` host. zsh history has an unrelated openvpn entry that also matches
    // "ue" weakly via the "compute" substring. After typing "ue", `ueransim` should
    // be the selected host and appear above the openvpn entry.
    let mut tui = HarnessBuilder::new()
        .file(
            ".ssh/config",
            "Include ~/.ssh/work\n\nHost workstation\n  HostName 10.0.0.1\n",
        )
        .file(
            ".ssh/work",
            "Host ueransim\n  HostName 10.10.10.10\n  User root\n",
        )
        .file(
            ".zsh_history",
            ": 1700000000:0;ssh -i \"~/.ssh/aws_gw.pem\" openvpnas@ec2-18-171-208-251.eu-west-2.compute.amazonaws.com\n",
        )
        .spawn();

    tui.send("ue");
    tui.settle();
    let screen = tui.screen();

    let ueransim_row = screen
        .iter()
        .position(|row| row.contains("ueransim"))
        .unwrap_or_else(|| panic!("ueransim should be visible. screen:\n{}", screen.join("\n")));

    if let Some(opv) = screen
        .iter()
        .position(|row| row.contains("ec2-18-171") || row.contains("openvpnas"))
    {
        assert!(
            ueransim_row < opv,
            "ueransim (row {ueransim_row}) should be above openvpn (row {opv}). screen:\n{}",
            screen.join("\n")
        );
    }

    let selected_row = tui
        .row_of("\u{25b8}")
        .unwrap_or_else(|| panic!("a host should be selected. screen:\n{}", screen.join("\n")));
    assert_eq!(
        selected_row, ueransim_row,
        "selection should be on ueransim. screen:\n{}",
        screen.join("\n")
    );

    tui.quit();
}
