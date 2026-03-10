use ratatui::widgets::ListState;

use crate::history::HistoryEntry;
use crate::tree::{ConfigNode, FlatItem, FlatItemKind};

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
}

/// Fuzzy match: checks if all pattern chars appear in text in order (not necessarily consecutive).
fn fuzzy_match(text: &str, pattern: &str, case_sensitive: bool) -> bool {
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
    pub fn new(root: ConfigNode, history: Vec<HistoryEntry>) -> Self {
        let mut all_items = Vec::new();

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
                        host_visible[i] = any_ancestor || matches(&item.label) || cmd_matches;
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
}
