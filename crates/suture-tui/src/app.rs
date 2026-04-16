//! Application state for the Suture TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use suture_common::{FileStatus, Hash};
use suture_core::repository::{RepoError, Repository};

use crate::event::key_matches;

/// Which tab/panel is currently focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Status,
    Log,
    Staging,
    Diff,
    Branches,
    Help,
}

impl Tab {
    /// All tabs in order for tab cycling.
    pub const ALL: [Tab; 6] = [Tab::Status, Tab::Log, Tab::Staging, Tab::Diff, Tab::Branches, Tab::Help];

    fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Status => "Status",
            Tab::Log => "Log",
            Tab::Staging => "Staging",
            Tab::Diff => "Diff",
            Tab::Branches => "Branches",
            Tab::Help => "Help",
        }
    }
}

/// A file entry in the staging area.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub status: FileStatus,
    pub staged: bool,
}

/// A log entry for the commit graph.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub message: String,
    pub timestamp: String,
    pub parents: Vec<String>,
    pub branch_heads: Vec<String>,
    pub is_merge: bool,
}

/// A diff line for display.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub line_type: DiffLineType,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    Context,
    Add,
    Remove,
    HunkHeader,
    ConflictMarker,
}

/// Action to perform on branch input submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchAction {
    Create,
    Rename,
}

/// Application state.
pub struct App {
    repo: Repository,

    // Current view
    current_tab: Tab,

    // Status data
    head_branch: Option<String>,
    head_patch: Option<String>,
    branch_count: usize,
    patch_count: usize,

    // File lists
    staged_files: Vec<FileEntry>,
    unstaged_files: Vec<FileEntry>,

    // Staging view
    staging_cursor: usize,
    staging_focus_staged: bool, // true = staged pane, false = unstaged pane
    staging_scroll: usize,      // scroll offset for long file lists

    // Log view
    log_entries: Vec<LogEntry>,
    log_cursor: usize,
    log_scroll: usize,

    // Diff view
    diff_lines: Vec<DiffLine>,
    diff_scroll: usize,
    diff_file: Option<String>,
    diff_path: Option<String>, // relative path for the file being diffed

    // Status bar
    status_message: String,
    error_message: Option<String>,

    // Branch view
    branch_list: Vec<(String, String)>, // (name, target_patch_hex)
    branch_cursor: usize,
    branch_input_mode: bool,
    branch_input: String,
    branch_input_action: BranchAction,

    // Commit message input
    commit_mode: bool,
    commit_message: String,

    /// Whether the app should quit.
    should_quit: bool,
}

impl App {
    pub fn new(repo: Repository) -> Self {
        Self {
            repo,
            current_tab: Tab::Status,
            head_branch: None,
            head_patch: None,
            branch_count: 0,
            patch_count: 0,
            staged_files: Vec::new(),
            unstaged_files: Vec::new(),
            staging_cursor: 0,
            staging_focus_staged: true,
            staging_scroll: 0,
            log_entries: Vec::new(),
            log_cursor: 0,
            log_scroll: 0,
            diff_lines: Vec::new(),
            diff_scroll: 0,
            diff_file: None,
            diff_path: None,
            status_message: String::new(),
            error_message: None,
            branch_list: Vec::new(),
            branch_cursor: 0,
            branch_input_mode: false,
            branch_input: String::new(),
            branch_input_action: BranchAction::Create,
            commit_mode: false,
            commit_message: String::new(),
            should_quit: false,
        }
    }

    /// Refresh data from the repository.
    pub fn refresh(&mut self) -> Result<(), RepoError> {
        self.error_message = None;

        // Refresh status
        let status = self.repo.status()?;
        self.head_branch = status.head_branch;
        self.head_patch = status.head_patch.map(|h| h.to_hex());
        self.branch_count = status.branch_count;
        self.patch_count = status.patch_count;

        // Refresh working set
        let working = self.repo.meta().working_set().map_err(RepoError::Meta)?;
        let staged_paths: std::collections::HashSet<String> =
            status.staged_files.iter().map(|(p, _)| p.clone()).collect();

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();

        for (path, file_status) in &working {
            let entry = FileEntry {
                path: path.clone(),
                status: *file_status,
                staged: staged_paths.contains(path),
            };
            if entry.staged {
                staged.push(entry);
            } else if *file_status != FileStatus::Clean {
                unstaged.push(entry);
            }
        }

        // Sort paths
        staged.sort_by(|a, b| a.path.cmp(&b.path));
        unstaged.sort_by(|a, b| a.path.cmp(&b.path));

        self.staged_files = staged;
        self.unstaged_files = unstaged;

        // Clamp cursor
        let max_staged = self.staged_files.len().saturating_sub(1);
        let max_unstaged = self.unstaged_files.len().saturating_sub(1);
        if self.staging_focus_staged && self.staging_cursor > max_staged {
            self.staging_cursor = max_staged;
        }
        if !self.staging_focus_staged && self.staging_cursor > max_unstaged {
            self.staging_cursor = max_unstaged;
        }

        // Refresh log
        self.refresh_log()?;

        // Refresh branch list
        self.refresh_branches()?;

        Ok(())
    }

    fn refresh_log(&mut self) -> Result<(), RepoError> {
        let patches = self.repo.log(None)?;
        let branch_map: std::collections::HashMap<String, Vec<String>> = self
            .repo
            .dag()
            .list_branches()
            .into_iter()
            .map(|(name, id)| (id.to_hex(), vec![name]))
            .fold(std::collections::HashMap::new(), |mut acc, (id, names)| {
                acc.entry(id).or_default().extend(names);
                acc
            });

        self.log_entries = patches
            .into_iter()
            .map(|p| {
                let is_merge = p.parent_ids.len() > 1;
                let parents: Vec<String> = p.parent_ids.iter().map(|id| id.to_hex()).collect();
                LogEntry {
                    id: p.id.to_hex(),
                    short_id: format!("{}…", &p.id.to_hex()[..12]),
                    author: p.author.clone(),
                    message: p.message.clone(),
                    timestamp: format_timestamp(p.timestamp),
                    parents,
                    branch_heads: branch_map.get(&p.id.to_hex()).cloned().unwrap_or_default(),
                    is_merge,
                }
            })
            .collect();

        let max_log = self.log_entries.len().saturating_sub(1);
        if self.log_cursor > max_log {
            self.log_cursor = max_log;
        }

        Ok(())
    }

    fn refresh_branches(&mut self) -> Result<(), RepoError> {
        self.branch_list = self
            .repo
            .dag()
            .list_branches()
            .into_iter()
            .map(|(name, id)| (name, id.to_hex()))
            .collect();
        self.branch_list.sort_by(|a, b| a.0.cmp(&b.0));
        let max = self.branch_list.len().saturating_sub(1);
        if self.branch_cursor > max {
            self.branch_cursor = max;
        }
        Ok(())
    }

    /// Handle a key event. Returns true if the app should quit.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.should_quit {
            return true;
        }

        // Branch input mode (for creating/renaming branches)
        if self.branch_input_mode {
            return self.handle_branch_input_key(key);
        }

        // Commit mode input
        if self.commit_mode {
            return self.handle_commit_key(key);
        }

        // Global keys
        if key_matches(key, KeyCode::Char('q'), KeyModifiers::NONE)
            || key_matches(key, KeyCode::Char('c'), KeyModifiers::CONTROL)
        {
            self.should_quit = true;
            return true;
        }

        if self.current_tab == Tab::Staging && key_matches(key, KeyCode::Tab, KeyModifiers::NONE) {
            return self.handle_staging_key(key);
        }

        if key_matches(key, KeyCode::Tab, KeyModifiers::NONE) {
            self.current_tab = self.current_tab.next();
            self.status_message = format!("Switched to {}", self.current_tab.title());
            return false;
        }

        if key_matches(key, KeyCode::BackTab, KeyModifiers::SHIFT) {
            self.current_tab = self.current_tab.prev();
            self.status_message = format!("Switched to {}", self.current_tab.title());
            return false;
        }

        // Number keys for quick tab switch
        if let KeyCode::Char(c) = key.code
            && key.modifiers.contains(KeyModifiers::ALT)
        {
            match c {
                '1' => self.current_tab = Tab::Status,
                '2' => self.current_tab = Tab::Log,
                '3' => self.current_tab = Tab::Staging,
                '4' => self.current_tab = Tab::Diff,
                '5' => self.current_tab = Tab::Branches,
                '6' => self.current_tab = Tab::Help,
                _ => {}
            }
            self.status_message = format!("Switched to {}", self.current_tab.title());
            return false;
        }

        // Tab-specific keys
        match self.current_tab {
            Tab::Status => self.handle_status_key(key),
            Tab::Log => self.handle_log_key(key),
            Tab::Staging => self.handle_staging_key(key),
            Tab::Diff => self.handle_diff_key(key),
            Tab::Branches => self.handle_branches_key(key),
            Tab::Help => self.handle_help_key(key),
        }
    }

    fn handle_status_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('s') => {
                self.current_tab = Tab::Staging;
                self.status_message = "Switched to Staging".to_string();
            }
            KeyCode::Char('l') => {
                self.current_tab = Tab::Log;
                self.status_message = "Switched to Log".to_string();
            }
            KeyCode::Char('b') => {
                self.current_tab = Tab::Branches;
                self.status_message = "Switched to Branches".to_string();
            }
            KeyCode::Char('c') => {
                if !self.staged_files.is_empty() {
                    self.commit_mode = true;
                    self.commit_message.clear();
                    self.status_message =
                        "Enter commit message (Enter to commit, Esc to cancel)".to_string();
                } else {
                    self.error_message = Some("Nothing staged to commit".to_string());
                }
            }
            KeyCode::Char('r') => {
                if let Err(e) = self.refresh() {
                    self.error_message = Some(format!("Refresh failed: {e}"));
                } else {
                    self.status_message = "Refreshed".to_string();
                }
            }
            _ => {}
        }
        false
    }

    fn handle_log_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.log_cursor > 0 {
                    self.log_cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.log_cursor < self.log_entries.len().saturating_sub(1) {
                    self.log_cursor += 1;
                }
            }
            KeyCode::Char('g') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.log_cursor = 0;
                    self.log_scroll = 0;
                }
            }
            KeyCode::Char('G') => {
                self.log_cursor = self.log_entries.len().saturating_sub(1);
            }
            KeyCode::Char('d') => {
                // Show diff for selected patch
                let patch_id = self.log_entries.get(self.log_cursor).map(|e| e.id.clone());
                if let Some(id) = patch_id {
                    self.show_patch_diff(&id);
                    self.current_tab = Tab::Diff;
                }
            }
            KeyCode::PageUp => {
                self.log_cursor = self.log_cursor.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.log_cursor =
                    (self.log_cursor + 10).min(self.log_entries.len().saturating_sub(1));
            }
            _ => {}
        }
        false
    }

    fn handle_staging_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.staging_cursor > 0 {
                    self.staging_cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = if self.staging_focus_staged {
                    self.staged_files.len()
                } else {
                    self.unstaged_files.len()
                };
                if self.staging_cursor < max.saturating_sub(1) {
                    self.staging_cursor += 1;
                }
            }
            KeyCode::PageUp => {
                self.staging_cursor = self.staging_cursor.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let max = if self.staging_focus_staged {
                    self.staged_files.len()
                } else {
                    self.unstaged_files.len()
                };
                self.staging_cursor = (self.staging_cursor + 10).min(max.saturating_sub(1));
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                self.toggle_staging();
            }
            KeyCode::Tab => {
                self.staging_focus_staged = !self.staging_focus_staged;
                self.staging_cursor = 0;
                self.status_message = if self.staging_focus_staged {
                    "Focus: Staged files".to_string()
                } else {
                    "Focus: Unstaged files".to_string()
                };
            }
            KeyCode::Char('a') => {
                // Stage all
                if let Err(e) = self.repo.add_all() {
                    self.error_message = Some(format!("Stage all failed: {e}"));
                } else if let Err(e) = self.refresh() {
                    self.error_message = Some(format!("Refresh failed: {e}"));
                } else {
                    self.status_message = "All files staged".to_string();
                }
            }
            KeyCode::Char('d') => {
                // Show diff for selected file
                self.show_file_diff();
            }
            KeyCode::Char('c') => {
                if !self.staged_files.is_empty() {
                    self.commit_mode = true;
                    self.commit_message.clear();
                    self.status_message =
                        "Enter commit message (Enter to commit, Esc to cancel)".to_string();
                } else {
                    self.error_message = Some("Nothing staged to commit".to_string());
                }
            }
            _ => {}
        }
        false
    }

    fn handle_commit_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                // Ctrl+Enter or plain Enter commits; Ctrl+J inserts newline
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+Enter: commit (alternative to plain Enter)
                    let msg = self.commit_message.trim().to_string();
                    if msg.is_empty() {
                        self.error_message = Some("Empty commit message".to_string());
                        return false;
                    }
                    if let Err(e) = self.repo.commit(&msg) {
                        self.error_message = Some(format!("Commit failed: {e}"));
                    } else if let Err(e) = self.refresh() {
                        self.error_message = Some(format!("Refresh failed: {e}"));
                    } else {
                        self.status_message = "Committed successfully".to_string();
                    }
                    self.commit_mode = false;
                    self.commit_message.clear();
                } else {
                    // Plain Enter: commit
                    let msg = self.commit_message.trim().to_string();
                    if msg.is_empty() {
                        self.error_message = Some("Empty commit message".to_string());
                        return false;
                    }
                    if let Err(e) = self.repo.commit(&msg) {
                        self.error_message = Some(format!("Commit failed: {e}"));
                    } else if let Err(e) = self.refresh() {
                        self.error_message = Some(format!("Refresh failed: {e}"));
                    } else {
                        self.status_message = "Committed successfully".to_string();
                    }
                    self.commit_mode = false;
                    self.commit_message.clear();
                }
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+J: insert newline in commit message
                self.commit_message.push('\n');
            }
            KeyCode::Esc => {
                self.commit_mode = false;
                self.commit_message.clear();
                self.status_message = "Commit cancelled".to_string();
            }
            KeyCode::Char(c) => {
                self.commit_message.push(c);
            }
            KeyCode::Backspace => {
                self.commit_message.pop();
            }
            _ => {}
        }
        false
    }

    fn handle_diff_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.diff_scroll = self.diff_scroll.saturating_add(1);
                let max = self.diff_lines.len().saturating_sub(1);
                if self.diff_scroll > max {
                    self.diff_scroll = max;
                }
            }
            KeyCode::Char('g') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.diff_scroll = 0;
                }
            }
            KeyCode::Char('G') => {
                self.diff_scroll = self.diff_lines.len().saturating_sub(1);
            }
            KeyCode::PageUp => {
                self.diff_scroll = self.diff_scroll.saturating_sub(20);
            }
            KeyCode::PageDown => {
                self.diff_scroll =
                    (self.diff_scroll + 20).min(self.diff_lines.len().saturating_sub(1));
            }
            _ => {}
        }
        false
    }

    fn handle_help_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    fn handle_branches_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.branch_cursor > 0 {
                    self.branch_cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.branch_cursor < self.branch_list.len().saturating_sub(1) {
                    self.branch_cursor += 1;
                }
            }
            KeyCode::Char('n') => {
                // Create new branch
                self.branch_input_mode = true;
                self.branch_input.clear();
                self.branch_input_action = BranchAction::Create;
                self.status_message = "Enter new branch name (Enter to confirm, Esc to cancel)".to_string();
            }
            KeyCode::Char('x') => {
                // Checkout selected branch
                let branch_name = self.branch_list.get(self.branch_cursor).map(|(n, _)| n.clone());
                if let Some(name) = branch_name {
                    if let Err(e) = self.repo.checkout(&name) {
                        self.error_message = Some(format!("Checkout failed: {e}"));
                    } else if let Err(e) = self.refresh() {
                        self.error_message = Some(format!("Refresh failed: {e}"));
                    } else {
                        self.status_message = format!("Checked out: {name}");
                    }
                }
            }
            KeyCode::Char('d') => {
                // Delete selected branch
                if let Some((name, _)) = self.branch_list.get(self.branch_cursor).cloned() {
                    // Don't allow deleting the current branch
                    if self.head_branch.as_deref() == Some(name.as_str()) {
                        self.error_message = Some("Cannot delete the current branch".to_string());
                        return false;
                    }
                    match self.repo.delete_branch(&name) {
                        Ok(()) => {
                            self.status_message = format!("Deleted branch: {name}");
                            if let Err(e) = self.refresh() {
                                self.error_message = Some(format!("Refresh failed: {e}"));
                            }
                        }
                        Err(e) => {
                            self.error_message = Some(format!("Delete failed: {e}"));
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                // Rename selected branch
                if let Some((name, _)) = self.branch_list.get(self.branch_cursor) {
                    self.branch_input_mode = true;
                    self.branch_input = name.clone();
                    self.branch_input_action = BranchAction::Rename;
                    self.status_message = "Enter new branch name (Enter to confirm, Esc to cancel)".to_string();
                }
            }
            KeyCode::Char('g') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.branch_cursor = 0;
                }
            }
            KeyCode::Char('G') => {
                self.branch_cursor = self.branch_list.len().saturating_sub(1);
            }
            _ => {}
        }
        false
    }

    fn handle_branch_input_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                let name = self.branch_input.trim().to_string();
                if name.is_empty() {
                    self.error_message = Some("Empty branch name".to_string());
                    return false;
                }
                // Validate branch name
                if suture_common::BranchName::new(&name).is_err() {
                    self.error_message = Some("Invalid branch name (must be non-empty, no null bytes)".to_string());
                    return false;
                }
                match self.branch_input_action {
                    BranchAction::Create => match self.repo.create_branch(&name, None) {
                        Ok(()) => {
                            self.status_message = format!("Created branch: {name}");
                            if let Err(e) = self.refresh() {
                                self.error_message = Some(format!("Refresh failed: {e}"));
                            }
                        }
                        Err(e) => self.error_message = Some(format!("Create branch failed: {e}")),
                    },
                    BranchAction::Rename => {
                        let old_name = self
                            .branch_list
                            .get(self.branch_cursor)
                            .map(|(n, _)| n.clone())
                            .unwrap_or_default();
                        // Delete old and create new (rename primitive)
                        if let Err(e) = self.repo.delete_branch(&old_name) {
                            self.error_message = Some(format!("Rename failed (delete): {e}"));
                        } else if let Err(e) = self.repo.create_branch(&name, None) {
                            self.error_message = Some(format!("Rename failed (create): {e}"));
                        } else {
                            self.status_message =
                                format!("Renamed: {old_name} → {name}");
                            if let Err(e) = self.refresh() {
                                self.error_message = Some(format!("Refresh failed: {e}"));
                            }
                        }
                    }
                }
                self.branch_input_mode = false;
                self.branch_input.clear();
            }
            KeyCode::Esc => {
                self.branch_input_mode = false;
                self.branch_input.clear();
                self.status_message = "Cancelled".to_string();
            }
            KeyCode::Char(c) => {
                self.branch_input.push(c);
            }
            KeyCode::Backspace => {
                self.branch_input.pop();
            }
            _ => {}
        }
        false
    }

    /// Toggle staging of the currently selected file.
    fn toggle_staging(&mut self) {
        let files = if self.staging_focus_staged {
            &self.staged_files
        } else {
            &self.unstaged_files
        };

        if let Some(entry) = files.get(self.staging_cursor) {
            if self.staging_focus_staged {
                // Unstage
                let repo_path = match suture_common::RepoPath::new(&entry.path) {
                    Ok(rp) => rp,
                    Err(_) => return,
                };
                if let Err(e) = self.repo.meta().working_set_remove(&repo_path) {
                    self.error_message = Some(format!("Unstage failed: {e}"));
                    return;
                }
                self.status_message = format!("Unstaged: {}", entry.path);
            } else {
                // Stage
                if let Err(e) = self.repo.add(&entry.path) {
                    self.error_message = Some(format!("Stage failed: {e}"));
                    return;
                }
                self.status_message = format!("Staged: {}", entry.path);
            }

            if let Err(e) = self.refresh() {
                self.error_message = Some(format!("Refresh failed: {e}"));
            }
        }
    }

    /// Show diff for the currently selected file in staging.
    fn show_file_diff(&mut self) {
        let file_path = if self.staging_focus_staged {
            self.staged_files
                .get(self.staging_cursor)
                .map(|e| e.path.clone())
        } else {
            self.unstaged_files
                .get(self.staging_cursor)
                .map(|e| e.path.clone())
        };

        if let Some(path) = file_path {
            self.diff_path = Some(path.clone());
            self.load_file_diff(&path);
            self.current_tab = Tab::Diff;
            self.diff_scroll = 0;
        }
    }

    /// Load diff content for a file path.
    fn load_file_diff(&mut self, path: &str) {
        self.diff_lines.clear();
        self.diff_file = Some(path.to_string());

        // Try to read the file from working tree
        let root = self.repo.root().to_path_buf();
        let full_path = root.join(path);

        // Read working tree version
        let working_content = std::fs::read_to_string(&full_path).unwrap_or_default();

        // Read HEAD version from CAS — extract the hash first to avoid borrow conflict
        let head_blob_hash = self
            .repo
            .snapshot_head()
            .ok()
            .and_then(|tree| tree.get(path).copied());

        let head_content = match head_blob_hash {
            Some(hash) => self
                .repo
                .cas()
                .get_blob(&hash)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok().filter(|s| !s.is_empty()))
                .unwrap_or_default(),
            None => String::new(),
        };

        // Compute line-level diff
        let old_lines: Vec<&str> = head_content.lines().collect();
        let new_lines: Vec<&str> = working_content.lines().collect();

        let hunks = compute_line_diff(&old_lines, &new_lines);
        for hunk in hunks {
            self.diff_lines.push(hunk);
        }

        if self.diff_lines.is_empty() {
            self.diff_lines.push(DiffLine {
                content: "(no changes)".to_string(),
                line_type: DiffLineType::Context,
                old_line: None,
                new_line: None,
            });
        }
    }

    /// Show diff for a specific patch.
    fn show_patch_diff(&mut self, patch_id_hex: &str) {
        let patch_id = match Hash::from_hex(patch_id_hex) {
            Ok(h) => h,
            Err(_) => {
                self.error_message = Some("Invalid patch ID".to_string());
                return;
            }
        };

        let patch = match self.repo.dag().get_patch(&patch_id) {
            Some(p) => p.clone(),
            None => {
                self.error_message = Some("Patch not found".to_string());
                return;
            }
        };

        self.diff_file = Some(format!("patch: {}", patch.message));
        self.diff_lines.clear();
        self.diff_path = None;

        // Get the parent tree paths and patch tree paths — extract data to avoid borrow conflicts
        let parent_id = patch.parent_ids.first().copied();
        let patch_id_copy = patch.id;

        let parent_paths: std::collections::HashSet<String> = match parent_id {
            Some(pid) => self
                .repo
                .snapshot(&pid)
                .ok()
                .map(|t| t.iter().map(|(k, _)| k.clone()).collect())
                .unwrap_or_default(),
            None => std::collections::HashSet::new(),
        };

        let patch_tree = self.repo.snapshot(&patch_id_copy).ok();
        let new_paths: std::collections::HashSet<String> = match &patch_tree {
            Some(t) => t.iter().map(|(k, _)| k.clone()).collect(),
            None => std::collections::HashSet::new(),
        };

        // Added files
        for path in new_paths.difference(&parent_paths) {
            self.diff_lines.push(DiffLine {
                content: format!("added: {path}"),
                line_type: DiffLineType::Add,
                old_line: None,
                new_line: None,
            });
        }

        // Deleted files
        for path in parent_paths.difference(&new_paths) {
            self.diff_lines.push(DiffLine {
                content: format!("deleted: {path}"),
                line_type: DiffLineType::Remove,
                old_line: None,
                new_line: None,
            });
        }

        // Modified files — need to compare hashes
        if let Some(ref tree) = patch_tree {
            let parent_tree = parent_id.and_then(|pid| self.repo.snapshot(&pid).ok());
            for path in new_paths.intersection(&parent_paths) {
                let new_hash = tree.get(path.as_str()).copied();
                let old_hash = parent_tree
                    .as_ref()
                    .and_then(|t| t.get(path.as_str()).copied());
                if old_hash != new_hash && (old_hash.is_some() || new_hash.is_some()) {
                    self.diff_lines.push(DiffLine {
                        content: format!("modified: {path}"),
                        line_type: DiffLineType::HunkHeader,
                        old_line: None,
                        new_line: None,
                    });
                }
            }
        }

        if self.diff_lines.is_empty() {
            self.diff_lines.push(DiffLine {
                content: "(no changes)".to_string(),
                line_type: DiffLineType::Context,
                old_line: None,
                new_line: None,
            });
        }
    }

    // --- Getters for the UI ---

    pub fn current_tab(&self) -> Tab {
        self.current_tab
    }
    pub fn head_branch(&self) -> Option<&str> {
        self.head_branch.as_deref()
    }
    pub fn head_patch(&self) -> Option<&str> {
        self.head_patch.as_deref()
    }
    pub fn branch_count(&self) -> usize {
        self.branch_count
    }
    pub fn patch_count(&self) -> usize {
        self.patch_count
    }
    pub fn staged_files(&self) -> &[FileEntry] {
        &self.staged_files
    }
    pub fn unstaged_files(&self) -> &[FileEntry] {
        &self.unstaged_files
    }
    pub fn log_entries(&self) -> &[LogEntry] {
        &self.log_entries
    }
    pub fn log_cursor(&self) -> usize {
        self.log_cursor
    }
    pub fn diff_lines(&self) -> &[DiffLine] {
        &self.diff_lines
    }
    pub fn diff_file(&self) -> Option<&str> {
        self.diff_file.as_deref()
    }
    pub fn diff_scroll(&self) -> usize {
        self.diff_scroll
    }
    pub fn status_message(&self) -> &str {
        &self.status_message
    }
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }
    pub fn commit_mode(&self) -> bool {
        self.commit_mode
    }
    pub fn commit_message(&self) -> &str {
        &self.commit_message
    }
    pub fn staging_cursor(&self) -> usize {
        self.staging_cursor
    }
    pub fn staging_focus_staged(&self) -> bool {
        self.staging_focus_staged
    }
    pub fn staging_scroll(&self) -> usize {
        self.staging_scroll
    }
    pub fn branch_list(&self) -> &[(String, String)] {
        &self.branch_list
    }
    pub fn branch_cursor(&self) -> usize {
        self.branch_cursor
    }
    pub fn branch_input_mode(&self) -> bool {
        self.branch_input_mode
    }
    pub fn branch_input(&self) -> &str {
        &self.branch_input
    }
    pub fn repo(&self) -> &Repository {
        &self.repo
    }
}

/// Format a unix timestamp to a human-readable date string.
fn format_timestamp(ts: u64) -> String {
    let secs = ts;
    // Days since epoch
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    // Compute year/month/day from days since epoch
    let (year, month, day) = days_to_date(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_date(mut days: i64) -> (i64, i64, i64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = if days >= 0 {
        days / 146097
    } else {
        (days - 146096) / 146097
    };
    let doe = days - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Simple line-level diff using LCS (Longest Common Subsequence).
fn compute_line_diff(old_lines: &[&str], new_lines: &[&str]) -> Vec<DiffLine> {
    let mut result = Vec::new();

    // Use a simple LCS-based approach for small files, fall back to per-line comparison
    if old_lines.is_empty() && new_lines.is_empty() {
        return result;
    }

    // Build edit script using LCS
    let lcs = lcs_table(old_lines, new_lines);
    let mut i = old_lines.len();
    let mut j = new_lines.len();
    let mut ops: Vec<(DiffLineType, usize, usize)> = Vec::new(); // (type, old_idx, new_idx)

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            ops.push((DiffLineType::Context, i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j] == lcs[i][j - 1]) {
            ops.push((DiffLineType::Add, i, j - 1));
            j -= 1;
        } else {
            ops.push((DiffLineType::Remove, i - 1, j));
            i -= 1;
        }
    }

    ops.reverse();

    // Convert to diff lines
    let mut old_line_no: usize;
    let mut new_line_no: usize;
    let mut in_hunk = false;

    for (line_type, old_idx, new_idx) in &ops {
        match line_type {
            DiffLineType::Context => {
                if !in_hunk && !ops.is_empty() {
                    in_hunk = true;
                }
                old_line_no = *old_idx + 1;
                new_line_no = *new_idx + 1;
                result.push(DiffLine {
                    content: old_lines[*old_idx].to_string(),
                    line_type: DiffLineType::Context,
                    old_line: Some(old_line_no),
                    new_line: Some(new_line_no),
                });
            }
            DiffLineType::Add => {
                new_line_no = *new_idx + 1;
                result.push(DiffLine {
                    content: new_lines[*new_idx].to_string(),
                    line_type: DiffLineType::Add,
                    old_line: None,
                    new_line: Some(new_line_no),
                });
            }
            DiffLineType::Remove => {
                old_line_no = *old_idx + 1;
                result.push(DiffLine {
                    content: old_lines[*old_idx].to_string(),
                    line_type: DiffLineType::Remove,
                    old_line: Some(old_line_no),
                    new_line: None,
                });
            }
            DiffLineType::HunkHeader | DiffLineType::ConflictMarker => {}
        }
    }

    result
}

/// Build LCS table for two slices.
fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let m = a.len() + 1;
    let n = b.len() + 1;
    let mut table = vec![vec![0usize; n]; m];

    for i in 1..m {
        for j in 1..n {
            if a[i - 1] == b[j - 1] {
                table[i][j] = table[i - 1][j - 1] + 1;
            } else {
                table[i][j] = table[i - 1][j].max(table[i][j - 1]);
            }
        }
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        // 0 = epoch
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
    }

    #[test]
    fn test_days_to_date_epoch() {
        let (y, m, d) = days_to_date(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_date_known() {
        // 2024-01-01 is day 19723 from epoch
        let (y, m, d) = days_to_date(19723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }

    #[test]
    fn test_compute_line_diff_empty() {
        let result = compute_line_diff(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_line_diff_add() {
        let result = compute_line_diff(&[], &["hello"]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].line_type, DiffLineType::Add);
    }

    #[test]
    fn test_compute_line_diff_remove() {
        let result = compute_line_diff(&["hello"], &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].line_type, DiffLineType::Remove);
    }

    #[test]
    fn test_compute_line_diff_unchanged() {
        let result = compute_line_diff(&["hello", "world"], &["hello", "world"]);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|l| l.line_type == DiffLineType::Context));
    }

    #[test]
    fn test_compute_line_diff_mixed() {
        let result = compute_line_diff(&["a", "b", "c"], &["a", "x", "c"]);
        assert!(result.iter().any(|l| l.line_type == DiffLineType::Add));
        assert!(result.iter().any(|l| l.line_type == DiffLineType::Remove));
        assert!(result.iter().any(|l| l.line_type == DiffLineType::Context));
    }

    #[test]
    fn test_tab_cycling() {
        let tab = Tab::Status;
        assert_eq!(tab.next(), Tab::Log);
        assert_eq!(Tab::Log.next(), Tab::Staging);
        assert_eq!(Tab::Help.next(), Tab::Status);
        assert_eq!(Tab::Status.prev(), Tab::Help);
    }

    #[test]
    fn test_tab_title() {
        assert_eq!(Tab::Status.title(), "Status");
        assert_eq!(Tab::Log.title(), "Log");
        assert_eq!(Tab::Help.title(), "Help");
    }

    fn make_test_app() -> App {
        let repo = Repository::open_in_memory().expect("open in-memory repo");
        App::new(repo)
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_handle_key_quit() {
        let mut app = make_test_app();
        assert!(app.handle_key(key(KeyCode::Char('q'), KeyModifiers::NONE)));
    }

    #[test]
    fn test_handle_key_tab_cycle() {
        let mut app = make_test_app();
        assert_eq!(app.current_tab(), Tab::Status);
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.current_tab(), Tab::Log);
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.current_tab(), Tab::Staging);
        // Tab on Staging toggles pane focus, not tab cycling
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.current_tab(), Tab::Staging);
        // Shift+Tab still cycles tabs from Staging
        app.handle_key(key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.current_tab(), Tab::Log);
        app.handle_key(key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.current_tab(), Tab::Status);
        app.handle_key(key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.current_tab(), Tab::Help);
    }

    #[test]
    fn test_handle_key_alt_number() {
        let mut app = make_test_app();
        app.handle_key(key(KeyCode::Char('3'), KeyModifiers::ALT));
        assert_eq!(app.current_tab(), Tab::Staging);
        app.handle_key(key(KeyCode::Char('1'), KeyModifiers::ALT));
        assert_eq!(app.current_tab(), Tab::Status);
        app.handle_key(key(KeyCode::Char('5'), KeyModifiers::ALT));
        assert_eq!(app.current_tab(), Tab::Branches);
        app.handle_key(key(KeyCode::Char('6'), KeyModifiers::ALT));
        assert_eq!(app.current_tab(), Tab::Help);
        app.handle_key(key(KeyCode::Char('2'), KeyModifiers::ALT));
        assert_eq!(app.current_tab(), Tab::Log);
        app.handle_key(key(KeyCode::Char('4'), KeyModifiers::ALT));
        assert_eq!(app.current_tab(), Tab::Diff);
    }

    #[test]
    fn test_handle_key_commit_requires_staged() {
        let mut app = make_test_app();
        app.handle_key(key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert_eq!(app.error_message(), Some("Nothing staged to commit"));
        assert!(!app.commit_mode());
    }

    #[test]
    fn test_handle_key_stage_toggle() {
        let mut app = make_test_app();
        app.current_tab = Tab::Staging;
        assert!(app.staging_focus_staged());
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert!(!app.staging_focus_staged());
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert!(app.staging_focus_staged());
    }

    #[test]
    fn test_handle_key_diff_scroll() {
        let mut app = make_test_app();
        app.diff_lines = (0..50)
            .map(|_| DiffLine {
                content: "line".to_string(),
                line_type: DiffLineType::Context,
                old_line: None,
                new_line: None,
            })
            .collect();
        app.current_tab = Tab::Diff;
        assert_eq!(app.diff_scroll(), 0);
        app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.diff_scroll(), 1);
        app.handle_key(key(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.diff_scroll(), 2);
        for _ in 0..100 {
            app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(app.diff_scroll(), 49);
    }

    #[test]
    fn test_commit_mode_enter_exit() {
        let mut app = make_test_app();
        app.staged_files.push(FileEntry {
            path: "test.txt".to_string(),
            status: FileStatus::Added,
            staged: true,
        });
        app.current_tab = Tab::Status;
        app.handle_key(key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(app.commit_mode());
        app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.commit_mode());
        assert_eq!(app.status_message(), "Commit cancelled");
    }

    #[test]
    fn test_commit_mode_submit() {
        let mut app = make_test_app();
        app.staged_files.push(FileEntry {
            path: "test.txt".to_string(),
            status: FileStatus::Added,
            staged: true,
        });
        app.current_tab = Tab::Status;
        app.handle_key(key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(app.commit_mode());
        app.handle_key(key(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(key(KeyCode::Char('i'), KeyModifiers::NONE));
        assert_eq!(app.commit_message(), "hi");
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.commit_mode());
    }

    #[test]
    fn test_commit_mode_empty_rejected() {
        let mut app = make_test_app();
        app.staged_files.push(FileEntry {
            path: "test.txt".to_string(),
            status: FileStatus::Added,
            staged: true,
        });
        app.current_tab = Tab::Status;
        app.handle_key(key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(app.commit_mode());
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.commit_mode());
        assert_eq!(app.error_message(), Some("Empty commit message"));
    }
}
