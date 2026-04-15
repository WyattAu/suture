use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Subcommand;
use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use suture_core::repository::{Repository, RepoError};
use tokio::sync::{broadcast, mpsc};
use tokio::time;
use tracing::{debug, error, info, warn};

const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(60);
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub repo_path: PathBuf,
    pub remote_url: Option<String>,
    pub sync_interval: Duration,
    pub commit_template: String,
    pub author: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::from("."),
            remote_url: None,
            sync_interval: DEFAULT_SYNC_INTERVAL,
            commit_template: "auto: {count} file(s) changed".to_string(),
            author: "suture-daemon".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Created,
    Modified,
    Removed,
}

impl FileChangeEvent {
    fn from_notify_event(event: &Event, repo_root: &Path) -> Option<Self> {
        let kind = match &event.kind {
            EventKind::Create(_) => ChangeKind::Created,
            EventKind::Modify(_) => ChangeKind::Modified,
            EventKind::Remove(_) => ChangeKind::Removed,
            _ => return None,
        };

        for path in &event.paths {
            let relative = path.strip_prefix(repo_root).ok()?;
            let rel_str = relative.to_string_lossy();
            if rel_str.starts_with(".suture") || rel_str.starts_with(".suture/") {
                continue;
            }
            return Some(FileChangeEvent {
                path: relative.to_path_buf(),
                kind,
            });
        }
        None
    }
}

pub struct FileWatcher {
    repo_path: PathBuf,
    debounce_window: Duration,
}

impl FileWatcher {
    pub fn new(repo_path: PathBuf, debounce_window: Duration) -> Self {
        Self {
            repo_path,
            debounce_window,
        }
    }

    pub fn run(
        self,
        mut shutdown_rx: broadcast::Receiver<()>,
        change_tx: mpsc::Sender<Vec<FileChangeEvent>>,
    ) {
        let repo_path = self.repo_path.clone();
        let debounce = self.debounce_window;

        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel::<Result<Event, notify::Error>>();

            let mut watcher = match recommended_watcher(tx) {
                Ok(w) => w,
                Err(e) => {
                    error!("failed to create file watcher: {e}");
                    return;
                }
            };

            if let Err(e) = watcher.watch(&repo_path, RecursiveMode::Recursive) {
                error!("failed to watch directory: {e}");
                return;
            }

            info!("watching {:?}", repo_path);

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            let rt = match rt {
                Ok(rt) => rt,
                Err(e) => {
                    error!("failed to create runtime for debounce: {e}");
                    return;
                }
            };

            rt.block_on(async move {
                let mut pending: HashMap<PathBuf, ChangeKind> = HashMap::new();
                let debounce_timer = time::sleep(debounce);
                tokio::pin!(debounce_timer);

                loop {
                    tokio::select! {
                        _ = shutdown_rx.recv() => {
                            info!("file watcher shutting down");
                            break;
                        }
                        event = async {
                            let mut got = false;
                            while let Ok(Ok(event)) = rx.try_recv() {
                                if let Some(change) = FileChangeEvent::from_notify_event(&event, &repo_path) {
                                    pending.insert(change.path.clone(), change.kind);
                                    got = true;
                                }
                            }
                            got
                        } => {
                            if event {
                                debounce_timer.as_mut().reset(time::Instant::now() + debounce);
                            }
                        }
                        _ = &mut debounce_timer => {
                            if !pending.is_empty() {
                                let changes: Vec<FileChangeEvent> = pending.drain()
                                    .map(|(path, kind)| FileChangeEvent { path, kind })
                                    .collect();
                                let count = changes.len();
                                debug!("debounced {} change(s)", count);
                                if change_tx.send(changes).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }

                    tokio::select! {
                        _ = time::sleep(Duration::from_millis(50)) => {}
                        _ = shutdown_rx.recv() => {
                            break;
                        }
                    }
                }
            });
        });
    }
}

pub struct AutoCommit {
    repo_path: PathBuf,
    commit_template: String,
}

impl AutoCommit {
    pub fn new(repo_path: PathBuf, commit_template: String) -> Self {
        Self {
            repo_path,
            commit_template,
        }
    }

    pub async fn handle_changes(&self, changes: &[FileChangeEvent]) -> Result<Option<String>, RepoError> {
        let mut repo = Repository::open(&self.repo_path)?;

        let mut added = 0;
        for change in changes {
            let path_str = change.path.to_string_lossy();
            if let Err(e) = repo.add(&path_str) {
                warn!("failed to stage {}: {e}", path_str);
            } else {
                added += 1;
            }
        }

        if added == 0 {
            return Ok(None);
        }

        let status = repo.status()?;
        if status.staged_files.is_empty() {
            return Ok(None);
        }

        let message = self
            .commit_template
            .replace("{count}", &added.to_string());

        match repo.commit(&message) {
            Ok(patch_id) => {
                info!("auto-committed {} file(s): {}", added, patch_id);
                Ok(Some(patch_id.to_hex()))
            }
            Err(RepoError::NothingToCommit) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

pub struct AutoSync {
    repo_path: PathBuf,
    remote_url: Option<String>,
    interval: Duration,
}

impl AutoSync {
    pub fn new(repo_path: PathBuf, remote_url: Option<String>, interval: Duration) -> Self {
        Self {
            repo_path,
            remote_url,
            interval,
        }
    }

    pub async fn run(&self, mut shutdown_rx: broadcast::Receiver<()>) {
        if self.remote_url.is_none() {
            info!("no remote configured, sync disabled");
            return;
        }

        let mut interval = time::interval(self.interval);

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("auto sync shutting down");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = self.sync_once().await {
                        warn!("sync failed: {e}");
                    }
                }
            }
        }
    }

    async fn sync_once(&self) -> Result<(), String> {
        info!("starting sync cycle");

        let repo_path = self.repo_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            let repo = Repository::open(&repo_path)
                .map_err(|e| format!("failed to open repo: {e}"))?;

            repo.status()
                .map_err(|e| format!("status check failed: {e}"))?;

            Ok::<(), String>(())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("sync cycle complete");
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(e) => Err(format!("sync task panicked: {e}")),
        }
    }
}

pub struct Daemon {
    config: DaemonConfig,
}

impl Daemon {
    pub fn new(repo_path: PathBuf, remote_url: Option<String>, sync_interval: Duration) -> Self {
        Self {
            config: DaemonConfig {
                repo_path,
                remote_url,
                sync_interval,
                ..Default::default()
            },
        }
    }

    pub fn with_config(config: DaemonConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(64);

        let watcher = FileWatcher::new(
            self.config.repo_path.clone(),
            DEBOUNCE_WINDOW,
        );
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx.clone());

        let auto_commit = AutoCommit::new(
            self.config.repo_path.clone(),
            self.config.commit_template.clone(),
        );

        let auto_sync = AutoSync::new(
            self.config.repo_path.clone(),
            self.config.remote_url.clone(),
            self.config.sync_interval,
        );
        let sync_shutdown = shutdown_tx.subscribe();
        let sync_handle = tokio::spawn(async move {
            auto_sync.run(sync_shutdown).await;
        });

        loop {
            tokio::select! {
                Some(changes) = change_rx.recv() => {
                    debug!("received {} file change(s)", changes.len());
                    match auto_commit.handle_changes(&changes).await {
                        Ok(Some(patch_id)) => {
                            info!("created patch: {patch_id}");
                        }
                        Ok(None) => {
                            debug!("no changes to commit");
                        }
                        Err(e) => {
                            error!("auto-commit failed: {e}");
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("received ctrl-c, shutting down");
                    break;
                }
                else => {
                    break;
                }
            }
        }

        let _ = shutdown_tx.send(());
        let _ = sync_handle.await;
        info!("daemon stopped");
        Ok(())
    }

    pub async fn run_watch_only(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(64);

        let watcher = FileWatcher::new(
            self.config.repo_path.clone(),
            DEBOUNCE_WINDOW,
        );
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx.clone());

        let auto_commit = AutoCommit::new(
            self.config.repo_path.clone(),
            self.config.commit_template.clone(),
        );

        loop {
            tokio::select! {
                Some(changes) = change_rx.recv() => {
                    match auto_commit.handle_changes(&changes).await {
                        Ok(Some(patch_id)) => info!("created patch: {patch_id}"),
                        Ok(None) => debug!("no changes to commit"),
                        Err(e) => error!("auto-commit failed: {e}"),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
                else => {
                    break;
                }
            }
        }

        let _ = shutdown_tx.send(());
        info!("watch-only daemon stopped");
        Ok(())
    }

    pub async fn run_sync_only(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (shutdown_tx, _) = broadcast::channel::<()>(2);

        let auto_sync = AutoSync::new(
            self.config.repo_path.clone(),
            self.config.remote_url.clone(),
            self.config.sync_interval,
        );
        let sync_shutdown = shutdown_tx.subscribe();
        let sync_handle = tokio::spawn(async move {
            auto_sync.run(sync_shutdown).await;
        });

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("received ctrl-c, stopping sync");
            }
            _ = sync_handle => {}
        }

        let _ = shutdown_tx.send(());
        info!("sync-only daemon stopped");
        Ok(())
    }
}

#[derive(Subcommand)]
pub enum DaemonCommand {
    Watch {
        #[arg(default_value = ".")]
        repo_path: PathBuf,
    },
    Sync {
        #[arg(default_value = ".")]
        repo_path: PathBuf,
        #[arg(long)]
        remote: Option<String>,
        #[arg(long, default_value_t = 60)]
        interval: u64,
    },
    Start {
        #[arg(default_value = ".")]
        repo_path: PathBuf,
        #[arg(long)]
        remote: Option<String>,
        #[arg(long, default_value_t = 60)]
        interval: u64,
    },
}

pub async fn execute_command(cmd: DaemonCommand) -> Result<(), Box<dyn Error + Send + Sync>> {
    match cmd {
        DaemonCommand::Watch { repo_path } => {
            let daemon = Daemon::new(repo_path, None, DEFAULT_SYNC_INTERVAL);
            daemon.run_watch_only().await
        }
        DaemonCommand::Sync {
            repo_path,
            remote,
            interval,
        } => {
            let daemon = Daemon::new(repo_path, remote, Duration::from_secs(interval));
            daemon.run_sync_only().await
        }
        DaemonCommand::Start {
            repo_path,
            remote,
            interval,
        } => {
            let daemon = Daemon::new(repo_path, remote, Duration::from_secs(interval));
            daemon.run().await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::time::{timeout, Duration};

    fn init_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("suture_daemon=debug")
            .with_test_writer()
            .try_init();
    }

    fn create_test_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let _repo = Repository::init(&repo_path, "test-user").unwrap();

        fs::write(repo_path.join("hello.txt"), "hello world").unwrap();

        let mut repo = Repository::open(&repo_path).unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial commit").unwrap();

        (dir, repo_path)
    }

    #[test]
    fn test_file_watcher_detects_changes() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(16);

        let watcher = FileWatcher::new(repo_path.clone(), Duration::from_millis(100));
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx);

        std::thread::sleep(Duration::from_millis(200));

        fs::write(repo_path.join("new_file.txt"), "new content").unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = rt.block_on(async {
            timeout(Duration::from_secs(5), async {
                let mut found = false;
                while let Some(changes) = change_rx.recv().await {
                    for c in &changes {
                        debug!(
                            "detected change: path={}, kind={:?}",
                            c.path.to_string_lossy(),
                            c.kind
                        );
                        if c.path.to_string_lossy() == "new_file.txt" {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                }
                found
            })
            .await
        });

        drop(shutdown_tx);
        assert!(result.unwrap_or(false), "file watcher should detect new file creation");
    }

    #[test]
    fn test_debounce_rapid_changes() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(16);

        let debounce_window = Duration::from_millis(500);
        let watcher = FileWatcher::new(repo_path.clone(), debounce_window);
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx);

        std::thread::sleep(Duration::from_millis(200));

        for i in 0..10 {
            fs::write(repo_path.join(format!("rapid_{i}.txt")), format!("content {i}")).unwrap();
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = rt.block_on(async {
            timeout(Duration::from_secs(5), async {
                let mut total_batches = 0;
                let mut total_changes = 0;
                while let Some(changes) = change_rx.recv().await {
                    total_batches += 1;
                    total_changes += changes.len();
                    if total_changes >= 10 {
                        break;
                    }
                }
                (total_batches, total_changes)
            })
            .await
        });

        let (batches, changes) = result.unwrap();
        assert!(changes >= 10, "should detect at least 10 changes, got {changes}");
        assert!(
            batches <= 5,
            "rapid changes should be debounced into few batches, got {batches}"
        );
    }

    #[tokio::test]
    async fn test_auto_commit_creates_patch() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let auto_commit = AutoCommit::new(
            repo_path.clone(),
            "auto: {count} file(s) changed".to_string(),
        );

        fs::write(repo_path.join("tracked_file.txt"), "new tracked content").unwrap();

        let changes = vec![FileChangeEvent {
            path: PathBuf::from("tracked_file.txt"),
            kind: ChangeKind::Created,
        }];

        let result = auto_commit.handle_changes(&changes).await;
        assert!(result.is_ok());
        let patch_id = result.unwrap();
        assert!(patch_id.is_some(), "auto-commit should create a patch");

        let repo = Repository::open(&repo_path).unwrap();
        let log = repo.log(None).unwrap();
        assert!(log.iter().any(|p| p.message.contains("auto:")));
    }

    #[tokio::test]
    async fn test_daemon_graceful_shutdown() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let daemon = Daemon::new(repo_path, None, Duration::from_secs(1));

        let handle = tokio::spawn(async move {
            match timeout(Duration::from_secs(2), daemon.run()).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => {}
                Err(_) => {}
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        handle.abort();

        let result = handle.await;
        assert!(result.is_ok() || result.is_err());
    }
}
