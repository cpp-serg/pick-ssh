use std::path::PathBuf;
use std::sync::mpsc;

use nucleo_matcher::{Config, Matcher, Utf32Str};
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
    /// When set, the main loop should suspend the TUI, open `$EDITOR` on the
    /// given file at the given 1-based line, then resume and reload state.
    pub edit_request: Option<(PathBuf, usize)>,
    /// Transient status message shown in the filter bar (e.g. "edit only available for config entries").
    pub status_msg: Option<String>,
    // Add-host form
    pub show_add_host: bool,
    pub add_form: AddHostForm,
    /// After saving a new host: alias to select + auto-test once config reloads.
    pub pending_select_and_test: Option<String>,
}

/// Field order in the add-host form. The numeric value is the active_field index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddHostField {
    Alias,
    HostName,
    User,
    Port,
    IdentityFile,
    ProxyJump,
    File,
}

impl AddHostField {
    pub const ALL: [AddHostField; 7] = [
        AddHostField::Alias,
        AddHostField::HostName,
        AddHostField::User,
        AddHostField::Port,
        AddHostField::IdentityFile,
        AddHostField::ProxyJump,
        AddHostField::File,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AddHostField::Alias => "Alias",
            AddHostField::HostName => "HostName",
            AddHostField::User => "User",
            AddHostField::Port => "Port",
            AddHostField::IdentityFile => "IdentityFile",
            AddHostField::ProxyJump => "ProxyJump",
            AddHostField::File => "File",
        }
    }
}

#[derive(Default)]
pub struct AddHostForm {
    pub alias: String,
    pub hostname: String,
    pub user: String,
    pub port: String,
    pub identity_file: String,
    pub proxy_jump: String,
    pub file: String,
    pub active_field: usize,
    pub error: Option<String>,
    /// Existing config files in the tree, used as suggestions when the File field is active.
    pub available_files: Vec<PathBuf>,
    /// Filtered subset of available_files (those that match the typed prefix).
    pub file_filtered: Vec<usize>,
    /// Index into file_filtered. usize::MAX means no suggestion is highlighted (use raw input).
    pub file_picker_idx: usize,
    /// When set, awaiting user y/n confirmation to create a new file at this path.
    pub confirm_create_file: Option<PathBuf>,
    /// If set, insert the new block immediately after this 1-based line in the file
    /// (used when duplicating an existing entry). None = append to end of file.
    pub insert_after_line: Option<usize>,
    /// When set, the form is in EDIT mode — saving will first delete the original
    /// block at this location, then write the new block.
    pub edit_original: Option<EditOriginal>,
    /// When true, the form is in MOVE mode (a stricter variant of EDIT where the
    /// user typically only changes the File field). Affects only the title.
    pub is_move: bool,
}

/// The original block being edited (for Ctrl+P / properties).
#[derive(Clone)]
pub struct EditOriginal {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

impl AddHostForm {
    pub fn field(&self, f: AddHostField) -> &str {
        match f {
            AddHostField::Alias => &self.alias,
            AddHostField::HostName => &self.hostname,
            AddHostField::User => &self.user,
            AddHostField::Port => &self.port,
            AddHostField::IdentityFile => &self.identity_file,
            AddHostField::ProxyJump => &self.proxy_jump,
            AddHostField::File => &self.file,
        }
    }

    pub fn field_mut(&mut self, f: AddHostField) -> &mut String {
        match f {
            AddHostField::Alias => &mut self.alias,
            AddHostField::HostName => &mut self.hostname,
            AddHostField::User => &mut self.user,
            AddHostField::Port => &mut self.port,
            AddHostField::IdentityFile => &mut self.identity_file,
            AddHostField::ProxyJump => &mut self.proxy_jump,
            AddHostField::File => &mut self.file,
        }
    }

    pub fn active(&self) -> AddHostField {
        AddHostField::ALL[self.active_field.min(AddHostField::ALL.len() - 1)]
    }
}

/// Fuzzy score using nucleo-matcher. Higher is better. Returns None if no match.
fn fuzzy_score(
    matcher: &mut Matcher,
    text: &str,
    pattern: &str,
    case_sensitive: bool,
) -> Option<u32> {
    matcher.config.ignore_case = !case_sensitive;
    let mut hay_buf = Vec::new();
    let mut needle_buf = Vec::new();
    let haystack = Utf32Str::new(text, &mut hay_buf);
    let needle = Utf32Str::new(pattern, &mut needle_buf);
    matcher.fuzzy_match(haystack, needle).map(u32::from)
}

/// Boolean fuzzy match. Used by SFTP filename filter where ranking isn't needed.
pub(crate) fn fuzzy_match(text: &str, pattern: &str, case_sensitive: bool) -> bool {
    let mut matcher = Matcher::new(Config::DEFAULT);
    fuzzy_score(&mut matcher, text, pattern, case_sensitive).is_some()
}

/// Exact (substring) score with smart case. Earlier match position = higher score.
fn exact_score(text: &str, pattern: &str, case_sensitive: bool) -> Option<u32> {
    let pos = if case_sensitive {
        text.find(pattern)?
    } else {
        let tl: String = text.chars().flat_map(|c| c.to_lowercase()).collect();
        let pl: String = pattern.chars().flat_map(|c| c.to_lowercase()).collect();
        tl.find(&pl)?
    };
    Some(u32::MAX - pos as u32)
}

impl App {
    pub fn new(
        root: ConfigNode,
        history: Vec<HistoryEntry>,
        known_hosts: Vec<String>,
        pinned: Vec<String>,
    ) -> Self {
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
                source_file: None,
                source_line: None,
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
                    source_file: None,
                    source_line: None,
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
                source_file: None,
                source_line: None,
            });
            for entry in &history[..recent_count] {
                all_items.push(FlatItem {
                    kind: FlatItemKind::Host,
                    label: entry.host.clone(),
                    depth: 1,
                    connect_host: Some(entry.host.clone()),
                    full_cmd: Some(entry.full_cmd.clone()),
                    info: None,
                    source_file: None,
                    source_line: None,
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
                source_file: None,
                source_line: None,
            });
            for entry in &history[recent_count..] {
                all_items.push(FlatItem {
                    kind: FlatItemKind::Host,
                    label: entry.host.clone(),
                    depth: 1,
                    connect_host: Some(entry.host.clone()),
                    full_cmd: Some(entry.full_cmd.clone()),
                    info: None,
                    source_file: None,
                    source_line: None,
                });
            }
        }

        // Known hosts section: entries from ~/.ssh/known_hosts not already represented
        // by an alias (config) or recent/history command. Goes last because it's the
        // largest noisiest source.
        let existing: std::collections::HashSet<String> = all_items
            .iter()
            .filter_map(|it| it.connect_host.clone())
            .collect();
        let new_known: Vec<String> = known_hosts
            .into_iter()
            .filter(|h| !existing.contains(h))
            .collect();
        if !new_known.is_empty() {
            all_items.push(FlatItem {
                kind: FlatItemKind::File,
                label: "Known".to_string(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            });
            for host in new_known {
                all_items.push(FlatItem {
                    kind: FlatItemKind::Host,
                    label: host.clone(),
                    depth: 1,
                    connect_host: Some(host),
                    full_cmd: None,
                    info: None,
                    source_file: None,
                    source_line: None,
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
            edit_request: None,
            status_msg: None,
            show_add_host: false,
            add_form: AddHostForm::default(),
            pending_select_and_test: None,
        }
    }

    /// Request the main loop to open $EDITOR on the selected host's config file.
    /// Sets a status message if the selected entry has no source file.
    pub fn start_edit(&mut self) {
        let Some(item) = self.selected_visible_item() else {
            return;
        };
        match (item.source_file.clone(), item.source_line) {
            (Some(file), Some(line)) => {
                self.edit_request = Some((file, line));
            }
            _ => {
                self.status_msg =
                    Some("Edit only available for config entries".to_string());
            }
        }
    }

    // ── Add-host form ────────────────────────────────────────────────

    /// Open the add-host form. Pre-populates available config files from the
    /// loaded tree and defaults the File field to the source of the currently
    /// selected entry (or `~/.ssh/config`).
    pub fn start_add_host(&mut self) {
        self.open_add_form_blank();
    }

    fn open_add_form_blank(&mut self) {
        let mut files: Vec<PathBuf> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in &self.all_items {
            if let Some(ref f) = item.source_file
                && seen.insert(f.clone())
            {
                files.push(f.clone());
            }
        }
        if files.is_empty()
            && let Some(home) = dirs::home_dir()
        {
            files.push(home.join(".ssh/config"));
        }

        let default_file = self
            .selected_visible_item()
            .and_then(|it| it.source_file.clone())
            .or_else(|| files.first().cloned())
            .map(|p| display_path(&p))
            .unwrap_or_default();

        self.add_form = AddHostForm {
            alias: String::new(),
            hostname: String::new(),
            user: String::new(),
            port: String::new(),
            identity_file: String::new(),
            proxy_jump: String::new(),
            file: default_file,
            active_field: 0,
            error: None,
            available_files: files,
            file_filtered: Vec::new(),
            file_picker_idx: usize::MAX,
            confirm_create_file: None,
            insert_after_line: None,
            edit_original: None,
            is_move: false,
        };
        self.show_add_host = true;
        self.add_host_recompute_file_filter();
    }

    /// Open the add-host form pre-filled with the currently selected config entry,
    /// so the user can save a duplicate (with an edited alias). The new block will
    /// be inserted immediately after the duplicated one in the same file.
    pub fn start_duplicate_host(&mut self) {
        let Some(item) = self.selected_visible_item() else {
            return;
        };
        // Config-defined entry: parse the source block and pre-fill all fields
        // including file + insert position so the copy lands next to the original.
        if let (Some(file), Some(line)) = (item.source_file.clone(), item.source_line)
            && let Some(parsed) = parse_host_block_at(&file, line)
        {
            self.open_add_form_blank();
            self.add_form.alias = format!("{}-copy", parsed.name);
            self.add_form.hostname = parsed.hostname.unwrap_or_default();
            self.add_form.user = parsed.user.unwrap_or_default();
            self.add_form.port = parsed.port.unwrap_or_default();
            self.add_form.identity_file = parsed.identity_file.unwrap_or_default();
            self.add_form.proxy_jump = parsed.proxy_jump.unwrap_or_default();
            self.add_form.file = display_path(&file);
            self.add_form.insert_after_line = Some(parsed.end_line);
            self.add_host_recompute_file_filter();
            return;
        }

        // Non-config entry (Recent / History / Known / Pinned): open the form with
        // whatever data we can derive from the entry. Default file is ~/.ssh/config
        // and insertion is in append mode (no insert_after_line).
        let label = item.label.clone();
        let connect = item.connect_host.clone().unwrap_or_default();
        let full_cmd = item.full_cmd.clone().unwrap_or_default();
        self.open_add_form_blank();
        // Parse user@host out of connect_host.
        let (user, hostname) = match connect.split_once('@') {
            Some((u, h)) => (u.to_string(), h.to_string()),
            None => (String::new(), connect.clone()),
        };
        self.add_form.alias = format!("{}-copy", sanitize_alias(&label));
        self.add_form.hostname = hostname;
        self.add_form.user = user;
        // Parse common ssh flags out of the full command, if any.
        let (port, ident, jump) = parse_ssh_flags(&full_cmd);
        self.add_form.port = port;
        self.add_form.identity_file = ident;
        self.add_form.proxy_jump = jump;
        // Default file = ~/.ssh/config, append mode (no insert_after_line).
        if let Some(home) = dirs::home_dir() {
            self.add_form.file = display_path(&home.join(".ssh/config"));
        }
        self.add_form.insert_after_line = None;
        self.add_host_recompute_file_filter();
    }

    /// Open the form in EDIT mode for the currently selected config entry. The
    /// fields are pre-filled with the parsed block; saving deletes the original
    /// and writes the new content (in-place if the file field is unchanged).
    pub fn start_edit_props(&mut self) {
        let Some(item) = self.selected_visible_item() else {
            return;
        };
        let (Some(file), Some(line)) = (item.source_file.clone(), item.source_line) else {
            self.status_msg =
                Some("Properties only available for config entries".to_string());
            return;
        };
        let Some(parsed) = parse_host_block_at(&file, line) else {
            self.status_msg = Some("Couldn't parse the selected host block".to_string());
            return;
        };

        self.open_add_form_blank();
        self.add_form.alias = parsed.name.clone();
        self.add_form.hostname = parsed.hostname.unwrap_or_default();
        self.add_form.user = parsed.user.unwrap_or_default();
        self.add_form.port = parsed.port.unwrap_or_default();
        self.add_form.identity_file = parsed.identity_file.unwrap_or_default();
        self.add_form.proxy_jump = parsed.proxy_jump.unwrap_or_default();
        self.add_form.file = display_path(&file);
        // Insert at the position where the original block starts. After the
        // original is deleted, this lands the new block in the same spot.
        self.add_form.insert_after_line = Some(line.saturating_sub(1));
        self.add_form.edit_original = Some(EditOriginal {
            file,
            start_line: line,
            end_line: parsed.end_line,
        });
        self.add_host_recompute_file_filter();
    }

    /// Open the form in MOVE mode for the currently selected config entry. Same
    /// flow as Ctrl+P but the File field is pre-focused and the title says "Move".
    pub fn start_move_host(&mut self) {
        let Some(item) = self.selected_visible_item() else {
            return;
        };
        if item.source_file.is_none() {
            self.status_msg = Some("Move only available for config entries".to_string());
            return;
        }
        self.start_edit_props();
        if !self.show_add_host {
            return;
        }
        self.add_form.is_move = true;
        self.add_form.active_field = AddHostField::ALL
            .iter()
            .position(|&f| f == AddHostField::File)
            .unwrap_or(0);
    }

    pub fn add_host_close(&mut self) {
        self.show_add_host = false;
        self.add_form = AddHostForm::default();
    }

    /// Called after a successful save: close the form and queue a reload+select+test
    /// for the main loop.
    pub fn finish_add_host(&mut self) {
        let alias = self.add_form.alias.trim().to_string();
        self.show_add_host = false;
        self.add_form = AddHostForm::default();
        if !alias.is_empty() {
            self.pending_select_and_test = Some(alias);
        }
    }

    pub fn add_host_cycle_field(&mut self, forward: bool) {
        let n = AddHostField::ALL.len();
        self.add_form.active_field = if forward {
            (self.add_form.active_field + 1) % n
        } else {
            (self.add_form.active_field + n - 1) % n
        };
    }

    pub fn add_host_input(&mut self, c: char) {
        let f = self.add_form.active();
        self.add_form.field_mut(f).push(c);
        self.add_form.error = None;
        if f == AddHostField::File {
            self.add_form.file_picker_idx = usize::MAX;
            self.add_host_recompute_file_filter();
        }
    }

    pub fn add_host_backspace(&mut self) {
        let f = self.add_form.active();
        self.add_form.field_mut(f).pop();
        self.add_form.error = None;
        if f == AddHostField::File {
            self.add_form.file_picker_idx = usize::MAX;
            self.add_host_recompute_file_filter();
        }
    }

    pub fn add_host_clear_field(&mut self) {
        let f = self.add_form.active();
        self.add_form.field_mut(f).clear();
        self.add_form.error = None;
        if f == AddHostField::File {
            self.add_form.file_picker_idx = usize::MAX;
            self.add_host_recompute_file_filter();
        }
    }

    /// Up/Down on the File field navigates the picker; on other fields cycles fields.
    pub fn add_host_nav_up(&mut self) {
        if self.add_form.active() == AddHostField::File && !self.add_form.file_filtered.is_empty() {
            let idx = self.add_form.file_picker_idx;
            if idx == usize::MAX {
                self.add_form.file_picker_idx = self.add_form.file_filtered.len() - 1;
            } else if idx > 0 {
                self.add_form.file_picker_idx = idx - 1;
            }
            self.add_host_apply_picker();
        } else {
            self.add_host_cycle_field(false);
        }
    }

    pub fn add_host_nav_down(&mut self) {
        if self.add_form.active() == AddHostField::File && !self.add_form.file_filtered.is_empty() {
            let idx = self.add_form.file_picker_idx;
            if idx == usize::MAX {
                self.add_form.file_picker_idx = 0;
            } else if idx + 1 < self.add_form.file_filtered.len() {
                self.add_form.file_picker_idx = idx + 1;
            }
            self.add_host_apply_picker();
        } else {
            self.add_host_cycle_field(true);
        }
    }

    fn add_host_apply_picker(&mut self) {
        if let Some(&i) = self
            .add_form
            .file_filtered
            .get(self.add_form.file_picker_idx)
            && let Some(p) = self.add_form.available_files.get(i)
        {
            self.add_form.file = display_path(p);
        }
    }

    fn add_host_recompute_file_filter(&mut self) {
        let needle = self.add_form.file.to_lowercase();
        let mut matches: Vec<usize> = Vec::new();
        for (i, p) in self.add_form.available_files.iter().enumerate() {
            let s = display_path(p).to_lowercase();
            if needle.is_empty() || s.contains(&needle) {
                matches.push(i);
            }
        }
        self.add_form.file_filtered = matches;
    }

    /// Returns the absolute path of the chosen file, expanding `~`.
    fn add_host_target_path(&self) -> PathBuf {
        expand_tilde(&self.add_form.file)
    }

    /// Validate the form. Returns Some(error message) or None.
    fn add_host_validate(&self) -> Option<String> {
        let alias = self.add_form.alias.trim();
        if alias.is_empty() {
            return Some("Alias is required".to_string());
        }
        if alias.contains(char::is_whitespace) {
            return Some("Alias must not contain whitespace".to_string());
        }
        // Conflict with any config-defined alias — but in edit mode, exclude the
        // entry being edited (matched by source_file + start_line).
        let edit_match = self.add_form.edit_original.as_ref();
        let conflict = self.all_items.iter().any(|it| {
            if it.source_file.is_none() || it.connect_host.as_deref() != Some(alias) {
                return false;
            }
            if let Some(eo) = edit_match
                && it.source_file.as_deref() == Some(eo.file.as_path())
                && it.source_line == Some(eo.start_line)
            {
                return false; // self
            }
            true
        });
        if conflict {
            return Some(format!("Alias '{alias}' already exists"));
        }
        if !self.add_form.port.trim().is_empty() {
            match self.add_form.port.trim().parse::<u32>() {
                Ok(p) if (1..=65535).contains(&p) => {}
                _ => return Some("Port must be 1-65535".to_string()),
            }
        }
        if self.add_form.file.trim().is_empty() {
            return Some("File path is required".to_string());
        }
        None
    }

    /// Try to save the form. Returns:
    /// - Ok(true) if saved.
    /// - Ok(false) if a confirmation is needed (file doesn't exist, set confirm_create_file).
    /// - Err with the message on validation failure (also sets self.add_form.error).
    pub fn add_host_try_save(&mut self) -> Result<bool, String> {
        if let Some(e) = self.add_host_validate() {
            self.add_form.error = Some(e.clone());
            return Err(e);
        }
        let target = self.add_host_target_path();
        if !target.exists() {
            self.add_form.confirm_create_file = Some(target);
            return Ok(false);
        }
        self.add_host_write(&target).map_err(|e| {
            let msg = format!("write failed: {e}");
            self.add_form.error = Some(msg.clone());
            msg
        })?;
        Ok(true)
    }

    /// Confirm creating a new file (after confirm_create_file was set).
    /// Creates parent dirs, then writes the entry.
    pub fn add_host_confirm_create(&mut self) -> Result<(), String> {
        let Some(target) = self.add_form.confirm_create_file.clone() else {
            return Ok(());
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create dirs failed: {e}"))?;
        }
        std::fs::write(&target, "").map_err(|e| format!("create file failed: {e}"))?;
        self.add_form.confirm_create_file = None;
        self.add_host_write(&target)
            .map_err(|e| format!("write failed: {e}"))
    }

    pub fn add_host_cancel_create(&mut self) {
        self.add_form.confirm_create_file = None;
    }

    fn add_host_write(&self, target: &std::path::Path) -> std::io::Result<()> {
        let block = self.format_host_block();

        // EDIT mode: delete the original block from its source file first. If the
        // target is the SAME file as the original, the deletion happens in-place
        // and `insert_after_line` (= original start_line - 1) correctly identifies
        // the position in the modified file. If the target is DIFFERENT, the
        // insert position becomes meaningless, so we fall back to append mode.
        let (effective_insert_after, source_target_same): (Option<usize>, bool) =
            if let Some(ref eo) = self.add_form.edit_original {
                let source_content = std::fs::read_to_string(&eo.file).unwrap_or_default();
                let modified = delete_lines(&source_content, eo.start_line, eo.end_line);
                std::fs::write(&eo.file, modified)?;
                let same = same_file(&eo.file, target);
                (
                    if same {
                        self.add_form.insert_after_line
                    } else {
                        None
                    },
                    same,
                )
            } else {
                (self.add_form.insert_after_line, false)
            };
        let _ = source_target_same; // silence unused (reserved for future use)

        let existing = std::fs::read_to_string(target).unwrap_or_default();

        let (before, after): (String, String) = match effective_insert_after {
            Some(after_line) => {
                let safe_line = advance_to_next_host_or_eof(&existing, after_line);
                let (b, a) = split_after_line(&existing, safe_line);
                (b.to_string(), a.to_string())
            }
            None => (existing, String::new()),
        };

        // Build output: previous content, exactly one blank line of separation
        // (if any previous content exists), the block, exactly one blank line of
        // separation (if more content follows), then the remaining content.
        let mut out = String::new();
        let trimmed_before = before.trim_end_matches('\n');
        if !trimmed_before.is_empty() {
            out.push_str(trimmed_before);
            out.push('\n'); // end previous line
            out.push('\n'); // blank line
        }
        out.push_str(&block);
        let trimmed_after = after.trim_start_matches('\n');
        if !trimmed_after.is_empty() {
            out.push('\n'); // blank line after block
            out.push_str(trimmed_after);
        }

        std::fs::write(target, out)?;
        Ok(())
    }

    fn format_host_block(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Host {}\n", self.add_form.alias.trim()));
        let push = |s: &mut String, key: &str, v: &str| {
            let v = v.trim();
            if !v.is_empty() {
                s.push_str(&format!("  {key} {v}\n"));
            }
        };
        push(&mut s, "HostName", &self.add_form.hostname);
        push(&mut s, "User", &self.add_form.user);
        push(&mut s, "Port", &self.add_form.port);
        push(&mut s, "IdentityFile", &self.add_form.identity_file);
        push(&mut s, "ProxyJump", &self.add_form.proxy_jump);
        s
    }

    /// Test the connection using the current form values. Builds an ssh argv that
    /// passes -o options derived from the fields, without needing the entry saved.
    pub fn add_host_test_connection(&mut self) {
        let host = if self.add_form.hostname.trim().is_empty() {
            self.add_form.alias.trim().to_string()
        } else {
            self.add_form.hostname.trim().to_string()
        };
        if host.is_empty() {
            self.add_form.error =
                Some("Need at least Alias or HostName to test".to_string());
            return;
        }
        let mut extra: Vec<String> = Vec::new();
        if !self.add_form.port.trim().is_empty() {
            extra.push("-p".into());
            extra.push(self.add_form.port.trim().to_string());
        }
        if !self.add_form.user.trim().is_empty() {
            extra.push("-l".into());
            extra.push(self.add_form.user.trim().to_string());
        }
        if !self.add_form.identity_file.trim().is_empty() {
            extra.push("-i".into());
            extra.push(self.add_form.identity_file.trim().to_string());
        }
        if !self.add_form.proxy_jump.trim().is_empty() {
            extra.push("-J".into());
            extra.push(self.add_form.proxy_jump.trim().to_string());
        }
        self.start_ssh_test_with(host, extra);
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

            let mut matcher = Matcher::new(Config::DEFAULT);
            let score = |matcher: &mut Matcher, text: &str| -> Option<u32> {
                if exact {
                    exact_score(text, &filter, case_sensitive)
                } else {
                    fuzzy_score(matcher, text, &filter, case_sensitive)
                }
            };

            // host_score: Some(s) means visible at score s. None means hidden.
            // Ancestor-only matches get score 0 so they sort to the bottom of the section.
            let mut host_score: Vec<Option<u32>> = vec![None; self.all_items.len()];
            let mut ancestor_stack: Vec<(usize, bool)> = Vec::new();

            for (i, item) in self.all_items.iter().enumerate() {
                match item.kind {
                    FlatItemKind::File => {
                        while ancestor_stack.last().is_some_and(|&(d, _)| d >= item.depth) {
                            ancestor_stack.pop();
                        }
                        let m = score(&mut matcher, &item.label).is_some();
                        ancestor_stack.push((item.depth, m));
                    }
                    FlatItemKind::Host => {
                        let any_ancestor = ancestor_stack.iter().any(|&(_, m)| m);
                        let label_score = score(&mut matcher, &item.label);
                        let cmd_score = item
                            .full_cmd
                            .as_ref()
                            .filter(|cmd| cmd.split_whitespace().count() > 2)
                            .and_then(|cmd| score(&mut matcher, cmd));
                        let info_score = item
                            .info
                            .as_ref()
                            .and_then(|info| score(&mut matcher, info));
                        let best = [label_score, cmd_score, info_score]
                            .into_iter()
                            .flatten()
                            .max();
                        host_score[i] = match (best, any_ancestor) {
                            (Some(s), _) => Some(s),
                            (None, true) => Some(0),
                            (None, false) => None,
                        };
                    }
                }
            }

            // Build visible list with hosts sorted by score (desc) within each File-run.
            // File headers separate sort groups, preserving the tree structure.
            let mut visible: Vec<usize> = Vec::new();
            let mut section: Vec<usize> = Vec::new();
            let flush = |visible: &mut Vec<usize>, section: &mut Vec<usize>, scores: &[Option<u32>]| {
                section.sort_by_key(|&i| std::cmp::Reverse(scores[i]));
                visible.append(section);
            };
            for (i, item) in self.all_items.iter().enumerate() {
                match item.kind {
                    FlatItemKind::File => {
                        flush(&mut visible, &mut section, &host_score);
                        visible.push(i);
                    }
                    FlatItemKind::Host => {
                        if host_score[i].is_some() {
                            section.push(i);
                        }
                    }
                }
            }
            flush(&mut visible, &mut section, &host_score);

            // Reorder sections (any File header starts a new section, regardless of
            // depth) by their best contained host score, descending. This lets a
            // deeply-nested Include with a strong match float to the top of the list.
            let mut chunks: Vec<(Option<u32>, Vec<usize>)> = Vec::new();
            let mut current: Vec<usize> = Vec::new();
            let mut current_best: Option<u32> = None;
            for &idx in &visible {
                let item = &self.all_items[idx];
                if matches!(item.kind, FlatItemKind::File) && !current.is_empty() {
                    chunks.push((current_best.take(), std::mem::take(&mut current)));
                }
                if let FlatItemKind::Host = item.kind
                    && let Some(s) = host_score[idx]
                {
                    current_best = Some(current_best.map_or(s, |c| c.max(s)));
                }
                current.push(idx);
            }
            if !current.is_empty() {
                chunks.push((current_best, current));
            }
            chunks.sort_by(|a, b| b.0.cmp(&a.0));
            self.visible = chunks.into_iter().flat_map(|(_, c)| c).collect();
            self.visible = self.prune_empty_headers();
        }

        // When filter is active, selection always snaps to the top match (the highest-
        // scoring host). When filter is empty, preserve selection by all_items index.
        if self.visible.is_empty() {
            self.selected = 0;
        } else if self.filter.is_empty() {
            if let Some(prev) = prev_idx {
                if let Some(pos) = self.visible.iter().position(|&idx| idx == prev) {
                    self.selected = pos;
                } else {
                    self.selected = self.find_closest_host(prev);
                }
            }
        } else {
            self.selected = self
                .visible
                .iter()
                .position(|&idx| matches!(self.all_items[idx].kind, FlatItemKind::Host))
                .unwrap_or(0);
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
        self.start_ssh_test_with(host, Vec::new());
    }

    /// Start an ssh -v test against `host`, prepending `extra_args` before the
    /// hostname. Used by both the host-list test and the add-host form's Ctrl+T.
    pub fn start_ssh_test_with(&mut self, host: String, extra_args: Vec<String>) {
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

            let mut args: Vec<String> = vec![
                "-v".into(),
                "-o".into(),
                "ConnectTimeout=5".into(),
                "-o".into(),
                "BatchMode=yes".into(),
            ];
            args.extend(extra_args);
            args.push(host);
            args.push("echo".into());
            args.push("ok".into());

            let child = std::process::Command::new("ssh")
                .args(&args)
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
                source_file: None,
                source_line: None,
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
                    source_file: None,
                    source_line: None,
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

fn display_path(p: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rel) = p.strip_prefix(&home)
    {
        return format!("~/{}", rel.display());
    }
    p.display().to_string()
}

fn expand_tilde(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(s)
}

/// Delete 1-based lines in the inclusive range [start, end] from `content`.
/// Always returns content with `\n`-terminated lines.
fn delete_lines(content: &str, start: usize, end: usize) -> String {
    let mut out = String::new();
    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if line_num < start || line_num > end {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Returns true iff `a` and `b` refer to the same file. Tries canonicalize first
/// for symlink/relative-path equivalence; falls back to byte equality.
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    if let (Ok(ca), Ok(cb)) = (a.canonicalize(), b.canonicalize()) {
        return ca == cb;
    }
    a == b
}

/// Make a string usable as an SSH config alias: replace any character that's not
/// alphanumeric, `-`, `_`, or `.` with `-`. Strips leading/trailing dashes.
fn sanitize_alias(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    while out.starts_with('-') {
        out.remove(0);
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Extract `-p PORT`, `-i IDENT`, and `-J JUMP` values from an ssh command line.
/// Returns `(port, identity_file, proxy_jump)` as String triples (empty if absent).
fn parse_ssh_flags(cmd: &str) -> (String, String, String) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let (mut port, mut ident, mut jump) = (String::new(), String::new(), String::new());
    let mut i = 0;
    while i + 1 < parts.len() {
        match parts[i] {
            "-p" => port = parts[i + 1].trim_matches('"').to_string(),
            "-i" => ident = parts[i + 1].trim_matches('"').to_string(),
            "-J" => jump = parts[i + 1].trim_matches('"').to_string(),
            _ => {}
        }
        i += 1;
    }
    (port, ident, jump)
}

/// Given a 1-based "insert after" line, advance forward through any non-directive
/// lines (properties, blanks, comments) until the next line is a `Host`/`Match`/
/// `Include` directive — or we run out of lines. Returns the new "insert after"
/// position. The result guarantees that inserting after this line lands either
/// right before a Host directive line, or at EOF.
fn advance_to_next_host_or_eof(content: &str, start_after_line: usize) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    let mut next_idx = start_after_line; // 0-indexed position of the line that would follow insertion
    while next_idx < lines.len() {
        let candidate = lines[next_idx];
        if line_starts_with_directive(candidate, "host")
            || line_starts_with_directive(candidate, "match")
            || line_starts_with_directive(candidate, "include")
        {
            return next_idx;
        }
        next_idx += 1;
    }
    next_idx
}

/// Split `content` so that `before` contains lines 1..=`after_line` (with their `\n`)
/// and `after` contains the remainder. If `after_line` is past EOF, returns
/// (content, ""). If `after_line` is 0, returns ("", content).
fn split_after_line(content: &str, after_line: usize) -> (&str, &str) {
    if after_line == 0 {
        return ("", content);
    }
    let mut count = 0usize;
    for (i, b) in content.bytes().enumerate() {
        if b == b'\n' {
            count += 1;
            if count == after_line {
                return (&content[..=i], &content[i + 1..]);
            }
        }
    }
    (content, "")
}

/// A parsed Host block from a config file, with the line number where it ends.
pub struct HostBlock {
    pub name: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<String>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
    /// 1-based line number of the last line that belongs to this block (the line
    /// just before the next `Host`/`Match` directive, or the last line of the file).
    pub end_line: usize,
}

pub fn parse_host_block_at(path: &std::path::Path, host_line: usize) -> Option<HostBlock> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let host_idx = host_line.checked_sub(1)?;
    let head = lines.get(host_idx)?;
    if !line_starts_with_directive(head, "host") {
        return None;
    }
    let alias = directive_value(head, "host")?
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();
    if alias.is_empty() {
        return None;
    }

    let mut block = HostBlock {
        name: alias,
        hostname: None,
        user: None,
        port: None,
        identity_file: None,
        proxy_jump: None,
        end_line: host_idx + 1,
    };

    for (i, line) in lines.iter().enumerate().skip(host_idx + 1) {
        // A new Host/Match/Include directive ends the block. Crucially, we use
        // word-boundary matching here so "HostName" is NOT mistaken for "Host".
        if line_starts_with_directive(line, "host")
            || line_starts_with_directive(line, "match")
            || line_starts_with_directive(line, "include")
        {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#')
            && let Some((key, value)) = trimmed.split_once(|c: char| c.is_whitespace() || c == '=')
        {
            let key_lower = key.to_lowercase();
            let value = value.trim_start_matches('=').trim();
            match key_lower.as_str() {
                "hostname" => block.hostname = Some(value.to_string()),
                "user" => block.user = Some(value.to_string()),
                "port" => block.port = Some(value.to_string()),
                "identityfile" => block.identity_file = Some(value.to_string()),
                "proxyjump" => block.proxy_jump = Some(value.to_string()),
                _ => {}
            }
        }
        block.end_line = i + 1;
    }

    Some(block)
}

/// True iff `line` (after leading whitespace) is the named SSH config directive
/// — i.e. the directive name followed by whitespace, `=`, or end-of-line. This
/// explicitly distinguishes `Host` from `HostName` (matching `"host"` does not
/// match `"hostname"` because the next char isn't whitespace/`=`/EOL).
pub fn line_starts_with_directive(line: &str, directive: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.len() < directive.len() {
        return false;
    }
    let (head, rest) = trimmed.split_at(directive.len());
    if !head.eq_ignore_ascii_case(directive) {
        return false;
    }
    matches!(rest.chars().next(), None | Some(' ' | '\t' | '='))
}

/// Returns the value portion of a directive line, with optional `=` stripped.
/// E.g. `directive_value("Host alpha", "host")` → `"alpha"`.
fn directive_value<'a>(line: &'a str, directive: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    let (_head, rest) = trimmed.split_at(directive.len());
    let r = rest.trim_start_matches('=').trim_start();
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_starts_with_directive_distinguishes_host_from_hostname() {
        assert!(line_starts_with_directive("Host alpha", "host"));
        assert!(line_starts_with_directive("  HOST alpha", "host"));
        assert!(line_starts_with_directive("host=alpha", "host"));
        assert!(line_starts_with_directive("Host\talpha", "host"));
        assert!(line_starts_with_directive("Host", "host"));
        // Critical: HostName is NOT a Host directive.
        assert!(!line_starts_with_directive("HostName 1.2.3.4", "host"));
        assert!(!line_starts_with_directive("  HostName=1.2.3.4", "host"));
        assert!(!line_starts_with_directive("hostname foo", "host"));
        // Match, MatchUser disambiguation
        assert!(line_starts_with_directive("Match user bob", "match"));
        assert!(!line_starts_with_directive("MatchUser bob", "match"));
    }

    #[test]
    fn parse_host_block_treats_hostname_as_property_not_boundary() {
        // Build a tempfile with a block whose only property is HostName, followed by
        // another Host. parse_host_block_at must consume the HostName line as part of
        // the alpha block and stop at "Host beta".
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(
            &path,
            "Host alpha\n  HostName 10.0.0.1\nHost beta\n  HostName 10.0.0.2\n",
        )
        .unwrap();
        let block = parse_host_block_at(&path, 1).expect("parsed");
        assert_eq!(block.name, "alpha");
        assert_eq!(block.hostname.as_deref(), Some("10.0.0.1"));
        // end_line is the last line of alpha's block (the HostName line at line 2),
        // so insertion lands BEFORE line 3 ("Host beta") — exactly where it should.
        assert_eq!(block.end_line, 2);
    }

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
    fn test_filter_ranks_better_match_first() {
        // Two hosts in the same section: one matches `prod` consecutively, the other scattered.
        // The consecutive match should sort first.
        let root = ConfigNode {
            path: std::path::PathBuf::from("config"),
            hosts: vec![],
            children: vec![],
        };
        let mut app = App::new(root, vec![], vec![], vec![]);
        app.all_items = vec![
            FlatItem {
                kind: FlatItemKind::File,
                label: "config".into(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
            FlatItem {
                kind: FlatItemKind::Host,
                label: "mongo-prod-replica".into(),
                depth: 1,
                connect_host: Some("mongo-prod-replica".into()),
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
            FlatItem {
                kind: FlatItemKind::Host,
                label: "prod-db".into(),
                depth: 1,
                connect_host: Some("prod-db".into()),
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
        ];
        app.visible = (0..app.all_items.len()).collect();

        app.filter = "prod".into();
        app.update_filter();

        // Visible: header + 2 hosts. The prefix match `prod-db` should come first.
        assert_eq!(app.visible.len(), 3);
        let host_names: Vec<&str> = app
            .visible
            .iter()
            .filter_map(|&i| match app.all_items[i].kind {
                FlatItemKind::Host => Some(app.all_items[i].label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(host_names, vec!["prod-db", "mongo-prod-replica"]);
    }

    #[test]
    fn test_filter_floats_top_section_with_best_match() {
        // Two top-level sections: "Recent" with a weak "ue" match, "config" with strong "ueransim" match.
        // Section containing the strongest match should come first.
        let root = ConfigNode {
            path: std::path::PathBuf::from("config"),
            hosts: vec![],
            children: vec![],
        };
        let mut app = App::new(root, vec![], vec![], vec![]);
        app.all_items = vec![
            FlatItem {
                kind: FlatItemKind::File,
                label: "Recent".into(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
            FlatItem {
                kind: FlatItemKind::Host,
                label: "queue-worker".into(),
                depth: 1,
                connect_host: Some("queue-worker".into()),
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
            FlatItem {
                kind: FlatItemKind::File,
                label: "config".into(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
            FlatItem {
                kind: FlatItemKind::Host,
                label: "ueransim".into(),
                depth: 1,
                connect_host: Some("ueransim".into()),
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
        ];
        app.visible = (0..app.all_items.len()).collect();

        app.filter = "ue".into();
        app.update_filter();

        // First visible item should be the "config" header, then "ueransim".
        assert!(app.visible.len() >= 2);
        assert_eq!(app.all_items[app.visible[0]].label, "config");
        assert_eq!(app.all_items[app.visible[1]].label, "ueransim");
    }

    #[test]
    fn test_filter_matches_info() {
        // Host whose alias is opaque but info contains the IP / jump host the user remembers.
        let root = ConfigNode {
            path: std::path::PathBuf::from("config"),
            hosts: vec![],
            children: vec![],
        };
        let mut app = App::new(root, vec![], vec![], vec![]);
        app.all_items = vec![
            FlatItem {
                kind: FlatItemKind::File,
                label: "config".into(),
                depth: 0,
                connect_host: None,
                full_cmd: None,
                info: None,
                source_file: None,
                source_line: None,
            },
            FlatItem {
                kind: FlatItemKind::Host,
                label: "main-a1".into(),
                depth: 1,
                connect_host: Some("main-a1".into()),
                full_cmd: None,
                info: Some("root@10.0.0.128/JumpHost.pem:bastion".into()),
                source_file: None,
                source_line: None,
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
