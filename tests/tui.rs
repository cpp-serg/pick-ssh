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
}

impl HarnessBuilder {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Add a file at `relative_path` (under fake $HOME) with the given content.
    fn file(mut self, relative_path: &str, content: impl Into<String>) -> Self {
        self.files.insert(PathBuf::from(relative_path), content.into());
        self
    }

    fn spawn(self) -> Tui {
        let home = TempDir::new().expect("tempdir");
        for (rel, content) in &self.files {
            let path = home.path().join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir parent");
            }
            fs::write(&path, content).expect("write fixture");
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
