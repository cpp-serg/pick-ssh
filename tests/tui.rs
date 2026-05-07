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
