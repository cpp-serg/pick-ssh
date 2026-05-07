use std::sync::mpsc;

use ratatui::widgets::ListState;

use crate::history::HistoryEntry;
use crate::pinned;
use crate::tree::{ConfigNode, FlatItem, FlatItemKind};

pub enum SshTestMsg {
    Line(String, String),      // (content, timestamp "ss:ms")
    Done(Option<i32>, String), // (exit code, timestamp)
    Error(String),
}

#[derive(Clone)]
pub struct SftpEntry {
    pub name: String,
    pub is_dir: bool,
}

pub enum SftpStatus {
    Connecting,
    Ready,
    Error(String),
}

pub enum SftpMsg {
    Entries(Vec<SftpEntry>),
    Error(String),
}

pub struct App {
    all_items: Vec<FlatItem>,
    pub visible: Vec<usize>,
    pub filter: String,
    pub selected: usize,
    pub list_state: ListState,
    pub page_size: usize,
    pub should_quit: bool,
    pub chosen_host: Option<String>,
    /// When a history entry is selected with Enter, contains the full original command
    pub chosen_full_cmd: Option<String>,
    /// Ctrl+Enter: print command instead of executing
    pub print_only: bool,
    pub theme_index: usize,
    pub theme_changed: bool,
    /// Top Y coordinate of list content area (inside border)
    pub list_area_y: u16,
    /// Height of list content area (inside border)
    pub list_area_height: u16,
    /// Pinned command lines (persisted to config file)
    pub pinned: Vec<String>,
    /// Whether to show "clear all pins?" confirmation
    pub confirm_clear_pins: bool,
    /// Whether to show the help popup
    pub show_help: bool,
    /// Whether to show the SSH test result popup
    pub show_ssh_test: bool,
    /// Host being tested
    pub ssh_test_host: String,
    /// Lines of SSH test output for the popup
    pub ssh_test_output: Vec<String>,
    /// Vertical scroll offset for SSH test popup
    pub ssh_test_scroll: u16,
    /// None = not running, Some(true) = success, Some(false) = failure
    pub ssh_test_result: Option<bool>,
    /// Timestamp when the SSH test completed
    pub ssh_test_done_ts: Option<String>,
    /// Channel receiver for streaming SSH test output
    pub ssh_test_rx: Option<mpsc::Receiver<SshTestMsg>>,
    // SFTP path completion popup
    pub show_sftp: bool,
    pub sftp_host: String,
    pub sftp_path_input: String,
    pub sftp_entries: Vec<SftpEntry>,
    pub sftp_filtered: Vec<usize>,
    pub sftp_selected: usize,
    pub sftp_status: SftpStatus,
    pub sftp_rx: Option<mpsc::Receiver<SftpMsg>>,
    pub sftp_last_dir: String,
    pub sftp_show_hidden: bool,
    pub sftp_list_state: ListState,
    pub sftp_chosen_path: Option<String>,
}

/// Fuzzy match: checks if all pattern chars appear in text in order (not necessarily consecutive).
pub(crate) fn fuzzy_match(text: &str, pattern: &str, case_sensitive: bool) -> bool {
    let mut pattern_chars = if case_sensitive {
        pattern.chars().peekable()
    } else {
        // We need owned iterators; collect then iterate
        return fuzzy_match_lower(text, pattern);
    };

    for tc in text.chars() {
        if let Some(&pc) = pattern_chars.peek() {
            if tc == pc {
                pattern_chars.next();
            }
        } else {
            break;
        }
    }
    pattern_chars.peek().is_none()
}

fn fuzzy_match_lower(text: &str, pattern: &str) -> bool {
    let text_lower: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
    let pattern_lower: Vec<char> = pattern.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut pi = 0;
    for &tc in &text_lower {
        if pi < pattern_lower.len() && tc == pattern_lower[pi] {
            pi += 1;
        }
    }
    pi == pattern_lower.len()
}

/// Exact (substring) match with smart case.
fn exact_match(text: &str, pattern: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        text.contains(pattern)
    } else {
        let text_lower: String = text.chars().flat_map(|c| c.to_lowercase()).collect();
        let pattern_lower: String = pattern.chars().flat_map(|c| c.to_lowercase()).collect();
        text_lower.contains(&pattern_lower)
    }
}

impl App {
    pub fn new(root: ConfigNode, history: Vec<HistoryEntry>, pinned: Vec<String>) -> Self {
        let mut all_items = Vec::new();

        // Pinned section
        if !pinned.is_empty() {
            all_items.push(FlatItem {
                kind: FlatItemKind::File,
                label: "Pinned".to_string(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
            });
            for cmd in &pinned {
                let host = cmd.split_whitespace().nth(1).unwrap_or(cmd).to_string();
                all_items.push(FlatItem {
                    kind: FlatItemKind::Host,
                    label: host.clone(),
                    depth: 1,
                    connect_host: Some(host),
                    full_cmd: Some(cmd.clone()),
                    info: None,
                });
            }
        }

        // Recent section: first 10 history entries
        let recent_count = history.len().min(10);
        if recent_count > 0 {
            all_items.push(FlatItem {
                kind: FlatItemKind::File,
                label: "Recent".to_string(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
            });
            for entry in &history[..recent_count] {
                all_items.push(FlatItem {
                    kind: FlatItemKind::Host,
                    label: entry.host.clone(),
                    depth: 1,
                    connect_host: Some(entry.host.clone()),
                    full_cmd: Some(entry.full_cmd.clone()),
                    info: None,
                });
            }
        }

        // SSH config nodes
        all_items.extend(root.flatten());

        // History section: remaining entries
        if history.len() > recent_count {
            all_items.push(FlatItem {
                kind: FlatItemKind::File,
                label: "History".to_string(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
            });
            for entry in &history[recent_count..] {
                all_items.push(FlatItem {
                    kind: FlatItemKind::Host,
                    label: entry.host.clone(),
                    depth: 1,
                    connect_host: Some(entry.host.clone()),
                    full_cmd: Some(entry.full_cmd.clone()),
                    info: None,
                });
            }
        }

        let visible: Vec<usize> = (0..all_items.len()).collect();
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App {
            all_items,
            visible,
            filter: String::new(),
            selected: 0,
            list_state,
            page_size: 10,
            should_quit: false,
            chosen_host: None,
            chosen_full_cmd: None,
            print_only: false,
            theme_index: 0,
            theme_changed: false,
            list_area_y: 0,
            list_area_height: 0,
            pinned,
            confirm_clear_pins: false,
            show_help: false,
            show_ssh_test: false,
            ssh_test_host: String::new(),
            ssh_test_output: Vec::new(),
            ssh_test_scroll: 0,
            ssh_test_result: None,
            ssh_test_done_ts: None,
            ssh_test_rx: None,
            show_sftp: false,
            sftp_host: String::new(),
            sftp_path_input: String::new(),
            sftp_entries: Vec::new(),
            sftp_filtered: Vec::new(),
            sftp_selected: 0,
            sftp_status: SftpStatus::Ready,
            sftp_rx: None,
            sftp_last_dir: String::new(),
            sftp_show_hidden: false,
            sftp_list_state: ListState::default(),
            sftp_chosen_path: None,
        }
    }

    pub fn items(&self) -> &[FlatItem] {
        &self.all_items
    }

    pub fn selected_visible_item(&self) -> Option<&FlatItem> {
        self.visible
            .get(self.selected)
            .map(|&idx| &self.all_items[idx])
    }

    /// Count unique visible hosts (by connect_host value).
    pub fn visible_host_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for &idx in &self.visible {
            let item = &self.all_items[idx];
            if matches!(item.kind, FlatItemKind::Host)
                && let Some(ref host) = item.connect_host
            {
                seen.insert(host.clone());
            }
        }
        seen.len()
    }

    pub fn update_filter(&mut self) {
        // Remember current selection's all_items index for preservation
        let prev_idx = self.visible.get(self.selected).copied();

        if self.filter.is_empty() {
            self.visible = (0..self.all_items.len()).collect();
        } else {
            // Leading ' switches to exact (substring) match
            let (exact, filter) = if let Some(stripped) = self.filter.strip_prefix('\'') {
                (true, stripped.to_string())
            } else {
                (false, self.filter.clone())
            };
            // Smart case: lowercase-only → case-insensitive; any uppercase → case-sensitive
            let case_sensitive = filter.chars().any(|c| c.is_uppercase());
            let matches = |text: &str| -> bool {
                if exact {
                    exact_match(text, &filter, case_sensitive)
                } else {
                    fuzzy_match(text, &filter, case_sensitive)
                }
            };

            // A host is visible if it matches OR any ancestor File header matches.
            let mut host_visible = vec![false; self.all_items.len()];
            let mut ancestor_stack: Vec<(usize, bool)> = Vec::new();

            for (i, item) in self.all_items.iter().enumerate() {
                match item.kind {
                    FlatItemKind::File => {
                        while ancestor_stack.last().is_some_and(|&(d, _)| d >= item.depth) {
                            ancestor_stack.pop();
                        }
                        ancestor_stack.push((item.depth, matches(&item.label)));
                    }
                    FlatItemKind::Host => {
                        let any_ancestor = ancestor_stack.iter().any(|&(_, m)| m);
                        let cmd_matches = item
                            .full_cmd
                            .as_ref()
                            .is_some_and(|cmd| cmd.split_whitespace().count() > 2 && matches(cmd));
                        let info_matches = item.info.as_ref().is_some_and(|info| matches(info));
                        host_visible[i] =
                            any_ancestor || matches(&item.label) || cmd_matches || info_matches;
                    }
                }
            }

            self.visible = self
                .all_items
                .iter()
                .enumerate()
                .filter(|(i, item)| match item.kind {
                    FlatItemKind::File => true,
                    FlatItemKind::Host => host_visible[*i],
                })
                .map(|(i, _)| i)
                .collect();

            self.visible = self.prune_empty_headers();
        }

        // Preserve selection: try to keep the same item selected, else find closest host
        if self.visible.is_empty() {
            self.selected = 0;
        } else if let Some(prev) = prev_idx {
            if let Some(pos) = self.visible.iter().position(|&idx| idx == prev) {
                self.selected = pos;
            } else {
                self.selected = self.find_closest_host(prev);
            }
        }

        self.sync_list_state();
    }

    /// Find the visible host whose all_items index is closest to `target_idx`.
    fn find_closest_host(&self, target_idx: usize) -> usize {
        let mut best_pos = 0;
        let mut best_dist = usize::MAX;

        for (pos, &idx) in self.visible.iter().enumerate() {
            if matches!(self.all_items[idx].kind, FlatItemKind::Host) {
                let dist = target_idx.abs_diff(idx);
                if dist < best_dist {
                    best_dist = dist;
                    best_pos = pos;
                }
            }
        }

        best_pos
    }

    fn prune_empty_headers(&self) -> Vec<usize> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < self.visible.len() {
            let idx = self.visible[i];
            let item = &self.all_items[idx];
            if matches!(item.kind, FlatItemKind::File) {
                let has_hosts = self.visible[i + 1..]
                    .iter()
                    .take_while(|&&j| !matches!(self.all_items[j].kind, FlatItemKind::File))
                    .any(|&j| matches!(self.all_items[j].kind, FlatItemKind::Host));
                if has_hosts {
                    result.push(idx);
                }
            } else {
                result.push(idx);
            }
            i += 1;
        }
        result
    }

    pub fn move_up(&mut self) {
        if self.selected == 0 {
            return;
        }
        self.selected -= 1;
        while self.selected > 0
            && self
                .visible
                .get(self.selected)
                .is_some_and(|&idx| matches!(self.all_items[idx].kind, FlatItemKind::File))
        {
            self.selected -= 1;
        }
        self.sync_list_state();
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 >= self.visible.len() {
            return;
        }
        self.selected += 1;
        while self.selected + 1 < self.visible.len()
            && self
                .visible
                .get(self.selected)
                .is_some_and(|&idx| matches!(self.all_items[idx].kind, FlatItemKind::File))
        {
            self.selected += 1;
        }
        self.sync_list_state();
    }

    /// Toggle exact/fuzzy mode by adding/removing leading `'`.
    pub fn toggle_exact(&mut self) {
        if let Some(stripped) = self.filter.strip_prefix('\'') {
            self.filter = stripped.to_string();
        } else {
            self.filter.insert(0, '\'');
        }
        self.update_filter();
    }

    /// Enter: select host, carrying full_cmd from history if available.
    pub fn select(&mut self) {
        if let Some(item) = self.selected_visible_item()
            && let FlatItemKind::Host = item.kind
        {
            let host = item.connect_host.clone();
            let cmd = item.full_cmd.clone();
            self.chosen_host = host;
            self.chosen_full_cmd = cmd;
            self.should_quit = true;
        }
    }

    /// Ctrl+Enter: select like Enter, but print command instead of executing.
    pub fn select_print_only(&mut self) {
        if let Some(item) = self.selected_visible_item()
            && let FlatItemKind::Host = item.kind
        {
            let host = item.connect_host.clone();
            let cmd = item.full_cmd.clone();
            self.chosen_host = host;
            self.chosen_full_cmd = cmd;
            self.print_only = true;
            self.should_quit = true;
        }
    }

    pub fn move_to_first(&mut self) {
        self.snap_to_first_host();
    }

    pub fn move_to_last(&mut self) {
        for (i, &idx) in self.visible.iter().enumerate().rev() {
            if matches!(self.all_items[idx].kind, FlatItemKind::Host) {
                self.selected = i;
                self.sync_list_state();
                return;
            }
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        for _ in 0..page_size {
            if self.selected == 0 {
                break;
            }
            self.selected -= 1;
            while self.selected > 0
                && self
                    .visible
                    .get(self.selected)
                    .is_some_and(|&idx| matches!(self.all_items[idx].kind, FlatItemKind::File))
            {
                self.selected -= 1;
            }
        }
        self.sync_list_state();
    }

    pub fn page_down(&mut self, page_size: usize) {
        for _ in 0..page_size {
            if self.selected + 1 >= self.visible.len() {
                break;
            }
            self.selected += 1;
            while self.selected + 1 < self.visible.len()
                && self
                    .visible
                    .get(self.selected)
                    .is_some_and(|&idx| matches!(self.all_items[idx].kind, FlatItemKind::File))
            {
                self.selected += 1;
            }
        }
        self.sync_list_state();
    }

    pub fn snap_to_first_host(&mut self) {
        for (i, &idx) in self.visible.iter().enumerate() {
            if matches!(self.all_items[idx].kind, FlatItemKind::Host) {
                self.selected = i;
                self.sync_list_state();
                return;
            }
        }
    }

    /// Handle a mouse click at the given screen row.
    /// Maps the row to a visible item index and selects it (skipping file headers).
    pub fn click_select(&mut self, row: u16) {
        if row < self.list_area_y || row >= self.list_area_y + self.list_area_height {
            return;
        }
        let offset = self.list_state.offset();
        let idx = (row - self.list_area_y) as usize + offset;
        if idx < self.visible.len() {
            let all_idx = self.visible[idx];
            if matches!(self.all_items[all_idx].kind, FlatItemKind::Host) {
                self.selected = idx;
                self.sync_list_state();
            }
        }
    }

    /// Toggle pin for the currently selected host entry.
    /// Pinned entries store the full command line (like Tab output).
    pub fn toggle_pin(&mut self) {
        let Some(item) = self.selected_visible_item() else {
            return;
        };
        if !matches!(item.kind, FlatItemKind::Host) {
            return;
        }
        let cmd = item
            .full_cmd
            .clone()
            .or_else(|| item.connect_host.as_ref().map(|h| format!("ssh {h}")))
            .unwrap_or_default();
        if cmd.is_empty() {
            return;
        }

        if let Some(pos) = self.pinned.iter().position(|p| *p == cmd) {
            self.pinned.remove(pos);
        } else {
            self.pinned.push(cmd);
        }
        pinned::save_pinned(&self.pinned);
        self.rebuild_items();
    }

    /// Returns the index into `self.pinned` if the selected item is a pinned entry.
    fn selected_pinned_index(&self) -> Option<usize> {
        let &all_idx = self.visible.get(self.selected)?;
        let item = &self.all_items[all_idx];
        if !matches!(item.kind, FlatItemKind::Host) {
            return None;
        }
        let cmd = item.full_cmd.as_ref().or(item.connect_host.as_ref())?;
        // Check the item is in the Pinned section (before the first non-Pinned File header)
        self.all_items
            .first()
            .filter(|h| matches!(h.kind, FlatItemKind::File) && h.label == "Pinned")?;
        // Pinned hosts are at all_items[1..=self.pinned.len()]
        if all_idx == 0 || all_idx > self.pinned.len() {
            return None;
        }
        let pin_idx = all_idx - 1;
        // Verify it actually matches
        if self.pinned.get(pin_idx).is_some_and(|p| p == cmd) {
            Some(pin_idx)
        } else {
            None
        }
    }

    /// Move the selected pinned item up in the pinned list.
    pub fn move_pin_up(&mut self) {
        let Some(pin_idx) = self.selected_pinned_index() else {
            return;
        };
        if pin_idx == 0 {
            return;
        }
        self.pinned.swap(pin_idx, pin_idx - 1);
        pinned::save_pinned(&self.pinned);
        // Track selection: it moved one position up in visible list
        self.rebuild_items_preserving_pin(pin_idx - 1);
    }

    /// Move the selected pinned item down in the pinned list.
    pub fn move_pin_down(&mut self) {
        let Some(pin_idx) = self.selected_pinned_index() else {
            return;
        };
        if pin_idx + 1 >= self.pinned.len() {
            return;
        }
        self.pinned.swap(pin_idx, pin_idx + 1);
        pinned::save_pinned(&self.pinned);
        self.rebuild_items_preserving_pin(pin_idx + 1);
    }

    /// Rebuild items and select the pinned entry at the given pinned index.
    fn rebuild_items_preserving_pin(&mut self, pin_idx: usize) {
        self.rebuild_items();
        // Pinned host at pin_idx is at all_items[pin_idx + 1] (after the header)
        let target_all_idx = pin_idx + 1;
        if let Some(pos) = self.visible.iter().position(|&idx| idx == target_all_idx) {
            self.selected = pos;
            self.sync_list_state();
        }
    }

    /// Clear all pinned entries.
    pub fn clear_pins(&mut self) {
        self.pinned.clear();
        pinned::save_pinned(&self.pinned);
        self.rebuild_items();
        self.confirm_clear_pins = false;
    }

    /// Start SSH connectivity test: spawn process, stream output via channel.
    pub fn start_ssh_test(&mut self) {
        let Some(item) = self.selected_visible_item() else {
            return;
        };
        if !matches!(item.kind, FlatItemKind::Host) {
            return;
        }
        let Some(host) = item.connect_host.clone() else {
            return;
        };

        self.ssh_test_host = host.clone();
        self.ssh_test_output = Vec::new();
        self.ssh_test_scroll = 0;
        self.ssh_test_result = None;
        self.ssh_test_done_ts = None;
        self.show_ssh_test = true;

        let (tx, rx) = mpsc::channel();
        self.ssh_test_rx = Some(rx);

        std::thread::spawn(move || {
            use std::io::BufRead;
            use std::time::Instant;

            let start = Instant::now();
            let ts = || {
                let d = start.elapsed();
                let secs = d.as_secs();
                let ms = d.subsec_millis();
                format!("{secs:02}:{ms:03}")
            };

            let child = std::process::Command::new("ssh")
                .args([
                    "-v",
                    "-o",
                    "ConnectTimeout=5",
                    "-o",
                    "BatchMode=yes",
                    &host,
                    "echo",
                    "ok",
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn();

            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(SshTestMsg::Error(format!("Failed to run ssh: {e}")));
                    return;
                }
            };

            let stderr = child.stderr.take().unwrap();
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(SshTestMsg::Line(l, ts())).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let status = child.wait();
            let code = status.ok().and_then(|s| s.code());
            let _ = tx.send(SshTestMsg::Done(code, ts()));
        });
    }

    /// Poll the SSH test channel for new output lines. Returns true if test is still active.
    pub fn poll_ssh_test(&mut self) -> bool {
        let Some(ref rx) = self.ssh_test_rx else {
            return false;
        };
        loop {
            match rx.try_recv() {
                Ok(SshTestMsg::Line(line, ts)) => {
                    self.ssh_test_output.push(format!("{ts}  {line}"));
                    // Auto-scroll to bottom while test is running
                    self.ssh_test_scroll = u16::MAX;
                }
                Ok(SshTestMsg::Done(code, ts)) => {
                    self.ssh_test_done_ts = Some(ts);
                    let success = code == Some(0);
                    self.ssh_test_result = Some(success);
                    self.ssh_test_scroll = u16::MAX;
                    self.ssh_test_rx = None;
                    return false;
                }
                Ok(SshTestMsg::Error(msg)) => {
                    self.ssh_test_output.push(msg);
                    self.ssh_test_result = Some(false);
                    self.ssh_test_scroll = u16::MAX;
                    self.ssh_test_rx = None;
                    return false;
                }
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.ssh_test_result = Some(false);
                    self.ssh_test_rx = None;
                    return false;
                }
            }
        }
    }

    // ── SFTP path completion ─────────────────────────────────────────

    pub fn start_sftp_completion(&mut self) {
        let Some(item) = self.selected_visible_item() else {
            return;
        };
        if !matches!(item.kind, FlatItemKind::Host) {
            return;
        }
        let Some(host) = item.connect_host.clone() else {
            return;
        };
        self.sftp_host = host;
        self.sftp_path_input = "./".to_string();
        self.sftp_entries.clear();
        self.sftp_filtered.clear();
        self.sftp_selected = 0;
        self.sftp_last_dir.clear();
        self.sftp_chosen_path = None;
        self.show_sftp = true;
        self.sftp_request_listing("./");
    }

    pub fn sftp_request_listing(&mut self, dir: &str) {
        self.sftp_status = SftpStatus::Connecting;
        self.sftp_last_dir = dir.to_string();
        self.sftp_entries.clear();
        self.sftp_filtered.clear();
        self.sftp_selected = 0;
        self.sftp_list_state.select(Some(0));

        let (tx, rx) = mpsc::channel();
        self.sftp_rx = Some(rx);

        let host = self.sftp_host.clone();
        let dir = dir.to_string();
        std::thread::spawn(move || {
            // Shell-escape the path with single quotes
            let escaped = dir.replace('\'', "'\\''");
            let ls_cmd = format!("ls -1pa '{escaped}'");
            let output = std::process::Command::new("ssh")
                .args([
                    "-o",
                    "ConnectTimeout=5",
                    "-o",
                    "BatchMode=yes",
                    &host,
                    &ls_cmd,
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output();

            match output {
                Ok(out) => {
                    if out.status.success() {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let mut entries = vec![SftpEntry {
                            name: "..".to_string(),
                            is_dir: true,
                        }];
                        entries.extend(
                            stdout
                                .lines()
                                .filter(|l| !l.is_empty() && *l != "./" && *l != "../")
                                .map(|l| {
                                    if let Some(name) = l.strip_suffix('/') {
                                        SftpEntry {
                                            name: name.to_string(),
                                            is_dir: true,
                                        }
                                    } else {
                                        SftpEntry {
                                            name: l.to_string(),
                                            is_dir: false,
                                        }
                                    }
                                }),
                        );
                        let _ = tx.send(SftpMsg::Entries(entries));
                    } else {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let msg = stderr.lines().last().unwrap_or("Unknown error").to_string();
                        let _ = tx.send(SftpMsg::Error(msg));
                    }
                }
                Err(e) => {
                    let _ = tx.send(SftpMsg::Error(format!("Failed to run ssh: {e}")));
                }
            }
        });
    }

    pub fn sftp_update_filter(&mut self) {
        let file_part = self
            .sftp_path_input
            .rsplit_once('/')
            .map(|(_, f)| f)
            .unwrap_or("");

        let is_hidden = |name: &str| name.starts_with('.') && name != "..";

        if file_part.is_empty() {
            let mut dirs: Vec<usize> = Vec::new();
            let mut files: Vec<usize> = Vec::new();
            for (i, e) in self.sftp_entries.iter().enumerate() {
                if !self.sftp_show_hidden && is_hidden(&e.name) {
                    continue;
                }
                if e.is_dir {
                    dirs.push(i);
                } else {
                    files.push(i);
                }
            }
            dirs.extend(files);
            self.sftp_filtered = dirs;
        } else {
            let case_sensitive = file_part.chars().any(|c| c.is_uppercase());
            let mut dirs: Vec<usize> = Vec::new();
            let mut files: Vec<usize> = Vec::new();
            for (i, e) in self.sftp_entries.iter().enumerate() {
                if !self.sftp_show_hidden && is_hidden(&e.name) {
                    continue;
                }
                if fuzzy_match(&e.name, file_part, case_sensitive) {
                    if e.is_dir {
                        dirs.push(i);
                    } else {
                        files.push(i);
                    }
                }
            }
            dirs.extend(files);
            self.sftp_filtered = dirs;
        }
        self.sftp_selected = 0;
        self.sftp_list_state.select(Some(0));
    }

    pub fn sftp_on_input_changed(&mut self) {
        let dir_part = match self.sftp_path_input.rfind('/') {
            Some(pos) => self.sftp_path_input[..=pos].to_string(),
            None => "./".to_string(),
        };
        if dir_part != self.sftp_last_dir {
            self.sftp_request_listing(&dir_part);
        }
        self.sftp_update_filter();
    }

    pub fn sftp_accept_selected(&mut self) {
        let Some(&entry_idx) = self.sftp_filtered.get(self.sftp_selected) else {
            return;
        };
        let entry = self.sftp_entries[entry_idx].clone();
        let dir_base = match self.sftp_path_input.rfind('/') {
            Some(pos) => self.sftp_path_input[..=pos].to_string(),
            None => "./".to_string(),
        };
        if entry.is_dir && entry.name == ".." {
            // Go up: remove last segment from dir_base
            let trimmed = dir_base.trim_end_matches('/');
            if let Some(pos) = trimmed.rfind('/') {
                self.sftp_path_input = trimmed[..=pos].to_string();
            } else {
                self.sftp_path_input = "/".to_string();
            }
            self.sftp_on_input_changed();
        } else if entry.is_dir {
            self.sftp_path_input = format!("{dir_base}{}/", entry.name);
            self.sftp_on_input_changed();
        } else {
            let full_path = format!("{dir_base}{}", entry.name);
            self.sftp_chosen_path = Some(format!("{}:{full_path}", self.sftp_host));
            self.show_sftp = false;
            self.should_quit = true;
        }
    }

    pub fn poll_sftp(&mut self) {
        let Some(ref rx) = self.sftp_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(SftpMsg::Entries(entries)) => {
                self.sftp_entries = entries;
                self.sftp_status = SftpStatus::Ready;
                self.sftp_rx = None;
                self.sftp_update_filter();
            }
            Ok(SftpMsg::Error(msg)) => {
                self.sftp_status = SftpStatus::Error(msg);
                self.sftp_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.sftp_status = SftpStatus::Error("Connection lost".to_string());
                self.sftp_rx = None;
            }
        }
    }

    /// Rebuild all_items and reapply filter after pinned list changes.
    fn rebuild_items(&mut self) {
        // Capture current selection's label to try to restore it
        let prev_label = self.selected_visible_item().map(|item| item.label.clone());

        // Strip old Pinned section: find where it starts and where the next section begins
        let pinned_start = self
            .all_items
            .iter()
            .position(|item| matches!(item.kind, FlatItemKind::File) && item.label == "Pinned");
        let rest_start = if let Some(start) = pinned_start {
            // Find the next File header after the Pinned header
            self.all_items[start + 1..]
                .iter()
                .position(|item| matches!(item.kind, FlatItemKind::File))
                .map(|p| start + 1 + p)
                .unwrap_or(self.all_items.len())
        } else {
            0
        };

        let non_pinned: Vec<FlatItem> = if pinned_start.is_some() {
            self.all_items[rest_start..].to_vec()
        } else {
            self.all_items.clone()
        };

        // Build new list with pinned section at top
        let mut new_items = Vec::new();
        if !self.pinned.is_empty() {
            new_items.push(FlatItem {
                kind: FlatItemKind::File,
                label: "Pinned".to_string(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
            });
            for cmd in &self.pinned {
                let host = cmd.split_whitespace().nth(1).unwrap_or(cmd).to_string();
                new_items.push(FlatItem {
                    kind: FlatItemKind::Host,
                    label: host.clone(),
                    depth: 1,
                    connect_host: Some(host),
                    full_cmd: Some(cmd.clone()),
                    info: None,
                });
            }
        }
        new_items.extend(non_pinned);
        self.all_items = new_items;
        self.update_filter();

        // Try to restore selection by label
        if let Some(label) = prev_label {
            for (i, &idx) in self.visible.iter().enumerate() {
                if self.all_items[idx].label == label
                    && matches!(self.all_items[idx].kind, FlatItemKind::Host)
                {
                    self.selected = i;
                    self.sync_list_state();
                    return;
                }
            }
        }
        self.snap_to_first_host();
    }

    fn sync_list_state(&mut self) {
        self.list_state.select(Some(self.selected));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_basic() {
        assert!(fuzzy_match("production", "prd", false));
        assert!(fuzzy_match("production", "prod", false));
        assert!(fuzzy_match("MainA1", "ma1", false));
        assert!(!fuzzy_match("production", "xyz", false));
    }

    #[test]
    fn test_fuzzy_match_case_sensitive() {
        assert!(fuzzy_match("MainA1", "MA1", true));
        assert!(!fuzzy_match("MainA1", "ma1", true));
    }

    #[test]
    fn test_fuzzy_match_empty_pattern() {
        assert!(fuzzy_match("anything", "", false));
        assert!(fuzzy_match("anything", "", true));
    }

    #[test]
    fn test_filter_matches_info() {
        // Host whose alias is opaque but info contains the IP / jump host the user remembers.
        let root = ConfigNode {
            path: std::path::PathBuf::from("config"),
            hosts: vec![],
            children: vec![],
        };
        let mut app = App::new(root, vec![], vec![]);
        app.all_items = vec![
            FlatItem {
                kind: FlatItemKind::File,
                label: "config".into(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
            },
            FlatItem {
                kind: FlatItemKind::Host,
                label: "main-a1".into(),
                depth: 1,
                connect_host: Some("main-a1".into()),
                full_cmd: None,
                info: Some("root@10.0.0.128/JumpHost.pem:bastion".into()),
            },
        ];
        app.visible = (0..app.all_items.len()).collect();

        // Match by IP fragment from info
        app.filter = "10.0".into();
        app.update_filter();
        assert_eq!(app.visible.len(), 2, "header + host should be visible");

        // Match by jump host from info
        app.filter = "bastion".into();
        app.update_filter();
        assert_eq!(app.visible.len(), 2);

        // Match by identity-file name
        app.filter = "JumpHost".into();
        app.update_filter();
        assert_eq!(app.visible.len(), 2);

        // Non-matching filter still hides the host
        app.filter = "nomatch".into();
        app.update_filter();
        assert_eq!(app.visible.len(), 0);
    }
}
