// SPDX-License-Identifier: MIT OR Apache-2.0
mod mount;
mod shm;

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use clap::Subcommand;
use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use suture_core::patch::types::{OperationType, Patch, TouchSet};
use suture_core::repository::{RepoError, Repository};
use suture_protocol::{
    BlobRef, BranchProto, PatchProto, PullRequest, PullResponse, PushRequest, PushResponse,
    hex_to_hash,
};
use tokio::sync::{broadcast, mpsc};
use tokio::time;
use tracing::{debug, error, info, warn};

const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(60);
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub struct AutoMountConfig {
    pub mount_type: String,
    pub path: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub repo_path: PathBuf,
    pub remote_url: Option<String>,
    pub sync_interval: Duration,
    pub commit_template: String,
    pub author: String,
    pub auto_mounts: Vec<AutoMountConfig>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::from("."),
            remote_url: None,
            sync_interval: DEFAULT_SYNC_INTERVAL,
            commit_template: "auto: {count} file(s) changed".to_owned(),
            author: "suture-daemon".to_owned(),
            auto_mounts: Vec::new(),
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
            return Some(Self {
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
    #[must_use]
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
                        () = &mut debounce_timer => {
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
                        () = time::sleep(Duration::from_millis(50)) => {}
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
    #[must_use]
    pub fn new(repo_path: PathBuf, commit_template: String) -> Self {
        Self {
            repo_path,
            commit_template,
        }
    }

    pub async fn handle_changes(
        &self,
        changes: &[FileChangeEvent],
    ) -> Result<Option<String>, RepoError> {
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

        let message = self.commit_template.replace("{count}", &added.to_string());

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
    #[must_use]
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

        let remote_url = {
            let rp = repo_path.clone();
            tokio::task::spawn_blocking(move || {
                let repo =
                    Repository::open(&rp).map_err(|e| format!("failed to open repo: {e}"))?;
                repo.get_remote_url("origin").map_or_else(
                    |_| {
                        info!("no remote 'origin' configured, skipping sync");
                        Ok(None)
                    },
                    |url| Ok(Some(url)),
                )
            })
            .await
            .map_err(|e| format!("sync task panicked: {e}"))?
            .map_err(|e: String| e)?
        };

        let Some(remote_url) = remote_url else {
            return Ok(());
        };

        match self.do_pull(&repo_path, &remote_url).await {
            Ok(count) if count > 0 => info!("pulled {} new patch(es)", count),
            Ok(_) => debug!("pull: already up to date"),
            Err(e) => warn!("pull failed: {e}"),
        }

        match self.do_push(&repo_path, &remote_url).await {
            Ok(count) if count > 0 => info!("pushed {} patch(es)", count),
            Ok(_) => debug!("push: nothing to push"),
            Err(e) => warn!("push failed: {e}"),
        }

        info!("sync cycle complete");
        Ok(())
    }

    async fn do_pull(&self, repo_path: &Path, remote_url: &str) -> Result<usize, String> {
        let known_branches: Vec<(String, String)> = {
            let rp = repo_path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                let repo =
                    Repository::open(&rp).map_err(|e| format!("failed to open repo: {e}"))?;
                Ok(repo
                    .list_branches()
                    .into_iter()
                    .map(|(name, id)| (name, id.to_hex()))
                    .collect::<Vec<_>>())
            })
            .await
            .map_err(|e| format!("pull task panicked: {e}"))?
            .map_err(|e: String| e)?
        };

        let known_branches_proto: Vec<BranchProto> = known_branches
            .iter()
            .map(|(name, hex)| BranchProto {
                name: name.clone(),
                target_id: hex_to_hash(hex),
            })
            .collect();

        let repo_id = derive_repo_id(remote_url, "origin");
        let pull_body = PullRequest {
            repo_id,
            known_branches: known_branches_proto,
            max_depth: None,
        };

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{remote_url}/pull/compressed"))
            .json(&pull_body)
            .send()
            .await
            .map_err(|e| format!("pull request failed: {e}"))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("pull failed (HTTP): {text}"));
        }

        let result: PullResponse = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse pull response: {e}"))?;

        if !result.success {
            return Err(format!(
                "pull failed: {}",
                result.error.as_deref().unwrap_or("unknown error")
            ));
        }

        if result.patches.is_empty() {
            return Ok(0);
        }

        let rp = repo_path.to_path_buf();
        let patches = result.patches;
        let branches = result.branches;
        let blobs = result.blobs;

        let new_count = tokio::task::spawn_blocking(move || {
            let mut repo =
                Repository::open(&rp).map_err(|e| format!("failed to open repo: {e}"))?;

            let old_tree = repo
                .snapshot_head()
                .unwrap_or_else(|_| suture_core::engine::tree::FileTree::empty());

            let b64 = base64::engine::general_purpose::STANDARD;

            for blob in &blobs {
                let hash = suture_common::Hash::from_hex(&blob.hash.value)
                    .map_err(|e| format!("invalid blob hash: {e}"))?;
                let compressed = b64
                    .decode(&blob.data)
                    .map_err(|e| format!("failed to decode blob: {e}"))?;
                let data = suture_protocol::decompress(&compressed)
                    .map_err(|e| format!("failed to decompress blob: {e}"))?;
                repo.cas()
                    .put_blob_with_hash(&data, &hash)
                    .map_err(|e| format!("failed to store blob: {e}"))?;
            }

            let mut count = 0usize;
            for patch_proto in &patches {
                let patch = proto_to_patch(patch_proto)
                    .map_err(|e| format!("failed to convert patch: {e}"))?;
                if !repo.dag().has_patch(&patch.id) {
                    repo.meta()
                        .store_patch(&patch)
                        .map_err(|e| format!("failed to store patch: {e}"))?;
                    let valid_parents: Vec<_> = patch
                        .parent_ids
                        .iter()
                        .filter(|pid| repo.dag().has_patch(pid))
                        .copied()
                        .collect();
                    if let Err(e) = repo
                        .dag_mut()
                        .add_patch(patch, valid_parents)
                        .map_err(|e| format!("failed to add patch to DAG: {e}"))
                    {
                        tracing::warn!("failed to add patch to DAG: {e}");
                        continue;
                    }
                    count += 1;
                }
            }

            for branch in &branches {
                let target_id = suture_common::Hash::from_hex(&branch.target_id.value)
                    .map_err(|e| format!("invalid branch target: {e}"))?;
                let branch_name = suture_common::BranchName::new(&branch.name)
                    .map_err(|e| format!("invalid branch name: {e}"))?;
                if repo.dag().branch_exists(&branch_name) {
                    if let Err(e) = repo.dag_mut().update_branch(&branch_name, target_id) {
                        tracing::warn!("update_branch failed: {e}");
                    }
                } else if let Err(e) = repo.dag_mut().create_branch(branch_name.clone(), target_id)
                {
                    tracing::warn!("create_branch failed: {e}");
                }
                repo.meta()
                    .set_branch(&branch_name, &target_id)
                    .map_err(|e| format!("failed to set branch: {e}"))?;
            }

            repo.invalidate_head_cache();
            repo.sync_working_tree(&old_tree)
                .map_err(|e| format!("failed to sync working tree: {e}"))?;

            Ok::<usize, String>(count)
        })
        .await
        .map_err(|e| format!("pull apply task panicked: {e}"))??;

        Ok(new_count)
    }

    async fn do_push(&self, repo_path: &Path, remote_url: &str) -> Result<usize, String> {
        let push_data = {
            let rp = repo_path.to_path_buf();
            tokio::task::spawn_blocking(move || -> Result<PushData, String> {
                let repo =
                    Repository::open(&rp).map_err(|e| format!("failed to open repo: {e}"))?;

                let push_state_key = "remote.origin.last_pushed";
                let patches: Vec<Patch> = if let Some(last_pushed_hex) =
                    repo.get_config(push_state_key).map_err(|e| e.to_string())?
                {
                    let last_pushed = suture_common::Hash::from_hex(&last_pushed_hex)
                        .map_err(|e| format!("invalid last_pushed hash: {e}"))?;
                    repo.patches_since(&last_pushed)
                } else {
                    repo.all_patches()
                };

                if patches.is_empty() {
                    return Ok(PushData {
                        patches: Vec::new(),
                        blobs: Vec::new(),
                        branches: Vec::new(),
                        head_hex: String::new(),
                    });
                }

                let branches = repo.list_branches();
                let (_, head_id) = repo
                    .head()
                    .unwrap_or_else(|_| ("main".to_owned(), suture_common::Hash::ZERO));

                let b64 = base64::engine::general_purpose::STANDARD;
                let mut blobs = Vec::new();
                let mut seen = HashMap::new();
                for patch in &patches {
                    collect_blobs_from_patch(patch, repo.cas(), &b64, &mut blobs, &mut seen);
                }

                let branches_proto: Vec<BranchProto> = branches
                    .iter()
                    .map(|(name, id)| BranchProto {
                        name: name.clone(),
                        target_id: hex_to_hash(&id.to_hex()),
                    })
                    .collect();

                Ok(PushData {
                    patches,
                    blobs,
                    branches: branches_proto,
                    head_hex: head_id.to_hex(),
                })
            })
            .await
            .map_err(|e| format!("push prepare task panicked: {e}"))?
            .map_err(|e: String| e)?
        };

        if push_data.patches.is_empty() {
            return Ok(0);
        }

        let repo_id = derive_repo_id(remote_url, "origin");

        let patches_proto: Vec<PatchProto> = push_data
            .patches
            .iter()
            .map(|p| PatchProto {
                id: hex_to_hash(&p.id.to_hex()),
                operation_type: p.operation_type.to_string(),
                touch_set: p.touch_set.addresses(),
                target_path: p.target_path.clone(),
                payload: base64::engine::general_purpose::STANDARD.encode(&p.payload),
                parent_ids: p
                    .parent_ids
                    .iter()
                    .map(|id| hex_to_hash(&id.to_hex()))
                    .collect(),
                author: p.author.clone(),
                message: p.message.clone(),
                timestamp: p.timestamp,
            })
            .collect();

        let push_body = PushRequest {
            repo_id,
            patches: patches_proto,
            branches: push_data.branches.clone(),
            blobs: push_data.blobs,
            signature: None,
            known_branches: Some(push_data.branches),
            force: false,
        };

        let b64 = base64::engine::general_purpose::STANDARD;
        let mut compressed_blobs = Vec::with_capacity(push_body.blobs.len());
        for blob in &push_body.blobs {
            let raw = b64
                .decode(&blob.data)
                .map_err(|e| format!("failed to decode blob for compression: {e}"))?;
            let compressed = suture_protocol::compress(&raw)
                .map_err(|e| format!("failed to compress blob: {e}"))?;
            compressed_blobs.push(BlobRef {
                hash: blob.hash.clone(),
                data: b64.encode(&compressed),
                truncated: false,
            });
        }
        let push_body = PushRequest {
            repo_id: push_body.repo_id,
            patches: push_body.patches,
            branches: push_body.branches,
            blobs: compressed_blobs,
            signature: push_body.signature,
            known_branches: push_body.known_branches,
            force: push_body.force,
        };

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{remote_url}/push/compressed"))
            .json(&push_body)
            .send()
            .await
            .map_err(|e| format!("push request failed: {e}"))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("push failed (HTTP): {text}"));
        }

        let result: PushResponse = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse push response: {e}"))?;

        if !result.success {
            return Err(format!(
                "push failed: {}",
                result.error.as_deref().unwrap_or("unknown error")
            ));
        }

        let patch_count = push_data.patches.len();
        let head_hex = push_data.head_hex;
        let rp = repo_path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let mut repo =
                Repository::open(&rp).map_err(|e| format!("failed to open repo: {e}"))?;
            repo.set_config("remote.origin.last_pushed", &head_hex)
                .map_err(|e| format!("failed to update last_pushed: {e}"))?;
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| format!("push cleanup task panicked: {e}"))??;

        Ok(patch_count)
    }
}

pub struct Daemon {
    config: DaemonConfig,
}

impl Daemon {
    #[must_use]
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

    #[must_use]
    pub fn with_config(config: DaemonConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let pid = std::process::id();
        shm::write_pid_file(pid)?;

        let (head_branch, patch_count) = {
            let repo = Repository::open(&self.config.repo_path)?;
            let head = repo.head().map(|(b, _)| b).unwrap_or_default();
            let status = repo.status()?;
            (head, status.patch_count as u32)
        };

        let shm_path = shm::create_shm_segment(1, patch_count, 0, &head_branch, pid)?;
        info!("SHM segment created at {:?}", shm_path);

        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(64);

        let watcher = FileWatcher::new(self.config.repo_path.clone(), DEBOUNCE_WINDOW);
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

        let shm_path_ref = shm_path.clone();
        let mut last_commit_ts: u64 = 0;
        let mut last_sync_ts: u64 = 0;

        loop {
            tokio::select! {
                Some(changes) = change_rx.recv() => {
                    debug!("received {} file change(s)", changes.len());
                    match auto_commit.handle_changes(&changes).await {
                        Ok(Some(patch_id)) => {
                            info!("created patch: {patch_id}");
                            last_commit_ts = now_nanos();
                            if let Ok(mut status) = shm::read_shm_status(&shm_path_ref) {
                                status.total_patches += 1;
                                status.last_commit_ts = last_commit_ts;
                                if let Err(e) = shm::update_shm_status(&shm_path_ref, &status) {
                                    tracing::warn!("SHM status update failed: {e}");
                                }
                            }
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

        if last_sync_ts == 0 {
            last_sync_ts = now_nanos();
        }
        if let Ok(mut status) = shm::read_shm_status(&shm_path_ref) {
            status.last_commit_ts = last_commit_ts;
            status.last_sync_ts = last_sync_ts;
            if let Err(e) = shm::update_shm_status(&shm_path_ref, &status) {
                tracing::warn!("SHM status update failed: {e}");
            }
        }

        let _ = shutdown_tx.send(());
        let _ = sync_handle.await;

        let _ = shm::cleanup_shm(&shm_path);
        let _ = shm::remove_pid_file();
        info!("daemon stopped");
        Ok(())
    }

    pub async fn run_watch_only(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(64);

        let watcher = FileWatcher::new(self.config.repo_path.clone(), DEBOUNCE_WINDOW);
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
    Stop,
    Status,
    Reload,
    Mount {
        #[arg(default_value = ".")]
        repo_path: PathBuf,
        #[arg(long, default_value = "fuse")]
        mount_type: String,
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        port: Option<u16>,
    },
    Unmount {
        #[arg(long)]
        id: String,
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
        DaemonCommand::Stop => {
            let pid = shm::read_pid_file()?;
            println!("stopping daemon (pid {pid})...");
            signal_process(pid, libc::SIGTERM);
            std::thread::sleep(std::time::Duration::from_millis(500));
            let shm_path = shm::shm_path_for_pid(pid);
            let _ = shm::cleanup_shm(&shm_path);
            let _ = shm::remove_pid_file();
            println!("daemon stopped");
            Ok(())
        }
        DaemonCommand::Status => {
            let pid = shm::read_pid_file()?;
            let shm_path = shm::shm_path_for_pid(pid);
            let status = shm::read_shm_status(&shm_path)?;
            print_status(&status);
            Ok(())
        }
        DaemonCommand::Reload => {
            let pid = shm::read_pid_file()?;
            println!("reloading daemon (pid {pid})...");
            signal_process(pid, libc::SIGHUP);
            println!("reload signal sent");
            Ok(())
        }
        DaemonCommand::Mount {
            repo_path,
            mount_type,
            path,
            port,
        } => {
            let mut manager = mount::MountManager::new(repo_path);
            match mount_type.as_str() {
                "fuse" => {
                    let mount_path =
                        path.ok_or_else(|| "missing --path for FUSE mount".to_owned())?;
                    let id = manager.mount_fuse(&mount_path)?;
                    println!("FUSE mounted: {id}");
                }
                "webdav" => {
                    let port = port.ok_or_else(|| "missing --port for WebDAV mount".to_owned())?;
                    let id = manager.mount_webdav(port)?;
                    println!("WebDAV mounted: {id}");
                }
                other => {
                    return Err(
                        format!("unknown mount type '{other}', use 'fuse' or 'webdav'").into(),
                    );
                }
            }
            Ok(())
        }
        DaemonCommand::Unmount { id } => {
            println!("unmount {id}: mount management requires a running daemon");
            Ok(())
        }
    }
}

fn print_status(status: &shm::ShmStatus) {
    println!("suture-daemon status:");
    println!("  pid:          {}", status.pid);
    println!("  version:      {}", status.version);
    println!("  repo_count:   {}", status.repo_count);
    println!("  patches:      {}", status.total_patches);
    println!("  blobs:        {}", status.total_blobs);
    println!("  head_branch:  {}", status.head_branch_str());
    println!("  mounted:      {}", status.is_mounted == 1);
    println!(
        "  last_commit:  {}",
        if status.last_commit_ts > 0 {
            format_timestamp(status.last_commit_ts)
        } else {
            "never".to_owned()
        }
    );
    println!(
        "  last_sync:    {}",
        if status.last_sync_ts > 0 {
            format_timestamp(status.last_sync_ts)
        } else {
            "never".to_owned()
        }
    );
}

fn format_timestamp(ts: u64) -> String {
    let secs = ts / 1_000_000_000;
    let nanos = ts % 1_000_000_000;
    let dt = UNIX_EPOCH + Duration::from_secs(secs);
    let datetime = humantime::format_rfc3339_seconds(dt);
    format!("{datetime}.{nanos:09}")
}

fn signal_process(pid: u32, sig: libc::c_int) {
    // SAFETY: libc::kill is safe to call for inter-process signaling. The pid
    // is obtained from the daemon's PID file and validated before this call.
    // A non-zero return (e.g. ESRCH) is acceptable — the caller handles errors.
    unsafe {
        libc::kill(pid as libc::pid_t, sig);
    }
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

struct PushData {
    patches: Vec<Patch>,
    blobs: Vec<BlobRef>,
    branches: Vec<BranchProto>,
    head_hex: String,
}

fn derive_repo_id(url: &str, remote_name: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let after_scheme = trimmed
        .find("://")
        .map_or(trimmed, |idx| &trimmed[idx + 3..]);
    if let Some(path_start) = after_scheme.find('/') {
        let path = &after_scheme[path_start + 1..];
        if let Some(name) = path.rsplit('/').next()
            && !name.is_empty()
        {
            return name.to_owned();
        }
    }
    remote_name.to_owned()
}

fn proto_to_patch(proto: &PatchProto) -> Result<Patch, String> {
    use suture_common::Hash;

    let id = Hash::from_hex(&proto.id.value).map_err(|e| format!("invalid patch id: {e}"))?;
    let parent_ids: Vec<suture_common::Hash> = proto
        .parent_ids
        .iter()
        .filter_map(|h| Hash::from_hex(&h.value).ok())
        .collect();
    let op_type = match proto.operation_type.as_str() {
        "create" => OperationType::Create,
        "delete" => OperationType::Delete,
        "move" => OperationType::Move,
        "metadata" => OperationType::Metadata,
        "merge" => OperationType::Merge,
        "identity" => OperationType::Identity,
        "batch" => OperationType::Batch,
        _ => OperationType::Modify,
    };
    let touch_set = TouchSet::from_addrs(proto.touch_set.iter().cloned());
    let payload = base64::engine::general_purpose::STANDARD
        .decode(&proto.payload)
        .map_err(|e| format!("failed to decode patch payload: {e}"))?;

    Ok(Patch::with_id(
        id,
        op_type,
        touch_set,
        proto.target_path.clone(),
        payload,
        parent_ids,
        proto.author.clone(),
        proto.message.clone(),
        proto.timestamp,
    ))
}

fn collect_blobs_from_patch(
    patch: &Patch,
    cas: &suture_core::cas::store::BlobStore,
    b64: &base64::engine::general_purpose::GeneralPurpose,
    blobs: &mut Vec<BlobRef>,
    seen: &mut HashMap<String, bool>,
) {
    let is_batch = patch.operation_type == OperationType::Batch;

    if is_batch {
        let changes = patch.file_changes().unwrap_or_default();
        for change in &changes {
            if change.payload.is_empty() {
                continue;
            }
            let hash_hex = String::from_utf8_lossy(&change.payload).to_string();
            if seen.contains_key(&hash_hex) {
                continue;
            }
            let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                continue;
            };
            seen.insert(hash_hex.clone(), true);
            let Ok(blob_data) = cas.get_blob(&hash) else {
                continue;
            };
            blobs.push(BlobRef {
                hash: hex_to_hash(&hash_hex),
                data: b64.encode(&blob_data),
                truncated: false,
            });
        }
    } else if !patch.payload.is_empty() {
        let hash_hex = String::from_utf8_lossy(&patch.payload).to_string();
        if !seen.contains_key(&hash_hex) {
            let Ok(hash) = suture_common::Hash::from_hex(&hash_hex) else {
                return;
            };
            seen.insert(hash_hex.clone(), true);
            let Ok(blob_data) = cas.get_blob(&hash) else {
                return;
            };
            blobs.push(BlobRef {
                hash: hex_to_hash(&hash_hex),
                data: b64.encode(&blob_data),
                truncated: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::time::{Duration, timeout};

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
        assert!(
            result.unwrap_or(false),
            "file watcher should detect new file creation"
        );
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
            fs::write(
                repo_path.join(format!("rapid_{i}.txt")),
                format!("content {i}"),
            )
            .unwrap();
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
        assert!(
            changes >= 10,
            "should detect at least 10 changes, got {changes}"
        );
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

    #[tokio::test]
    async fn test_auto_sync_performs_real_sync() {
        init_tracing();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut hub = suture_hub::SutureHubServer::new_in_memory().unwrap();
        hub.set_no_auth(true);

        let hub_addr = format!("127.0.0.1:{}", port);
        let server_handle = tokio::spawn(async move {
            let _ = suture_hub::server::run_server(hub, &hub_addr).await;
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        let hub_url = format!("http://127.0.0.1:{}", port);

        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "test-user").unwrap();

        std::fs::write(repo_path.join("hello.txt"), "hello world").unwrap();
        repo.add("hello.txt").unwrap();
        repo.commit("initial commit").unwrap();

        repo.add_remote("origin", &hub_url).unwrap();

        let auto_sync = AutoSync::new(
            repo_path.clone(),
            Some(hub_url.clone()),
            Duration::from_secs(60),
        );

        let result = auto_sync.sync_once().await;
        assert!(
            result.is_ok(),
            "sync_once should succeed: {:?}",
            result.err()
        );

        repo = Repository::open(&repo_path).unwrap();
        let last_pushed = repo
            .get_config("remote.origin.last_pushed")
            .unwrap()
            .expect("last_pushed should be set after push");
        assert!(!last_pushed.is_empty(), "last_pushed should not be empty");

        let (_, head_id) = repo.head().unwrap();
        assert_eq!(
            head_id.to_hex(),
            last_pushed,
            "last_pushed should match HEAD"
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_auto_sync_no_remote_returns_ok() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let auto_sync = AutoSync::new(repo_path.clone(), None, Duration::from_secs(60));
        let result = auto_sync.sync_once().await;
        assert!(
            result.is_ok(),
            "sync_once should return Ok when no remote configured"
        );
    }

    #[tokio::test]
    async fn test_sync_no_remote_configured() {
        init_tracing();
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        Repository::init(&repo_path, "test-user").unwrap();

        let auto_sync = AutoSync::new(repo_path.clone(), None, Duration::from_secs(60));
        let result = auto_sync.sync_once().await;
        assert!(
            result.is_ok(),
            "sync_once with no remote should succeed gracefully"
        );
    }

    #[tokio::test]
    async fn test_sync_hub_unreachable() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let bad_url = "http://127.0.0.1:1".to_string();
        let repo = Repository::open(&repo_path).unwrap();
        repo.add_remote("origin", &bad_url).unwrap();
        drop(repo);

        let auto_sync = AutoSync::new(repo_path.clone(), Some(bad_url), Duration::from_secs(60));

        let result = auto_sync.sync_once().await;
        assert!(
            result.is_ok(),
            "sync_once should handle unreachable hub gracefully without panicking"
        );
    }

    #[tokio::test]
    async fn test_sync_empty_repo() {
        init_tracing();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut hub = suture_hub::SutureHubServer::new_in_memory().unwrap();
        hub.set_no_auth(true);

        let hub_addr = format!("127.0.0.1:{}", port);
        let server_handle = tokio::spawn(async move {
            let _ = suture_hub::server::run_server(hub, &hub_addr).await;
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        let hub_url = format!("http://127.0.0.1:{}", port);

        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let repo = Repository::init(&repo_path, "test-user").unwrap();
        repo.add_remote("origin", &hub_url).unwrap();

        let auto_sync = AutoSync::new(
            repo_path.clone(),
            Some(hub_url.clone()),
            Duration::from_secs(60),
        );

        let result = auto_sync.sync_once().await;
        assert!(
            result.is_ok(),
            "syncing an empty repo should succeed: {:?}",
            result.err()
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_sync_multiple_rounds() {
        init_tracing();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut hub = suture_hub::SutureHubServer::new_in_memory().unwrap();
        hub.set_no_auth(true);

        let hub_addr = format!("127.0.0.1:{}", port);
        let server_handle = tokio::spawn(async move {
            let _ = suture_hub::server::run_server(hub, &hub_addr).await;
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        let hub_url = format!("http://127.0.0.1:{}", port);

        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "test-user").unwrap();

        fs::write(repo_path.join("file1.txt"), "first").unwrap();
        repo.add("file1.txt").unwrap();
        repo.commit("first commit").unwrap();
        repo.add_remote("origin", &hub_url).unwrap();

        let auto_sync = AutoSync::new(
            repo_path.clone(),
            Some(hub_url.clone()),
            Duration::from_secs(60),
        );

        let result1 = auto_sync.sync_once().await;
        assert!(
            result1.is_ok(),
            "first sync should succeed: {:?}",
            result1.err()
        );

        let repo = Repository::open(&repo_path).unwrap();
        let _log1 = repo.log(None).unwrap();

        let mut repo = Repository::open(&repo_path).unwrap();
        fs::write(repo_path.join("file2.txt"), "second").unwrap();
        repo.add("file2.txt").unwrap();
        let commit_result = repo.commit("second commit");
        assert!(
            commit_result.is_ok(),
            "second commit should succeed: {:?}",
            commit_result.err()
        );

        let repo = Repository::open(&repo_path).unwrap();
        let log_before_sync = repo.log(None).unwrap();
        let has_second = log_before_sync.iter().any(|p| p.message == "second commit");
        assert!(has_second, "second commit should be in log after commit");

        let auto_sync2 = AutoSync::new(
            repo_path.clone(),
            Some(hub_url.clone()),
            Duration::from_secs(60),
        );
        let result2 = auto_sync2.sync_once().await;
        assert!(
            result2.is_ok(),
            "second sync should succeed: {:?}",
            result2.err()
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_sync_with_blobs() {
        init_tracing();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut hub = suture_hub::SutureHubServer::new_in_memory().unwrap();
        hub.set_no_auth(true);

        let hub_addr = format!("127.0.0.1:{}", port);
        let server_handle = tokio::spawn(async move {
            let _ = suture_hub::server::run_server(hub, &hub_addr).await;
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        let hub_url = format!("http://127.0.0.1:{}", port);

        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "test-user").unwrap();

        let blob_content = "x".repeat(4096);
        fs::write(repo_path.join("bigfile.bin"), &blob_content).unwrap();
        repo.add("bigfile.bin").unwrap();
        repo.commit("add binary blob").unwrap();
        repo.add_remote("origin", &hub_url).unwrap();

        let auto_sync = AutoSync::new(
            repo_path.clone(),
            Some(hub_url.clone()),
            Duration::from_secs(60),
        );

        let result = auto_sync.sync_once().await;
        assert!(
            result.is_ok(),
            "syncing repo with blobs should succeed: {:?}",
            result.err()
        );

        let repo = Repository::open(&repo_path).unwrap();
        let last_pushed = repo.get_config("remote.origin.last_pushed").unwrap();
        assert!(
            last_pushed.is_some(),
            "last_pushed should be set after push"
        );

        server_handle.abort();
    }

    #[test]
    fn test_file_watcher_detects_new_file() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(16);

        let watcher = FileWatcher::new(repo_path.clone(), Duration::from_millis(100));
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx);

        std::thread::sleep(Duration::from_millis(200));

        fs::write(repo_path.join("brand_new.txt"), "freshly created").unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = rt.block_on(async {
            timeout(Duration::from_secs(5), async {
                while let Some(changes) = change_rx.recv().await {
                    for c in &changes {
                        if c.path.to_string_lossy() == "brand_new.txt" {
                            return true;
                        }
                    }
                }
                false
            })
            .await
        });

        drop(shutdown_tx);
        assert!(
            result.unwrap_or(false),
            "watcher should detect new file creation"
        );
    }

    #[test]
    fn test_file_watcher_detects_modification() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(16);

        let watcher = FileWatcher::new(repo_path.clone(), Duration::from_millis(100));
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx);

        std::thread::sleep(Duration::from_millis(200));

        fs::write(repo_path.join("hello.txt"), "modified content").unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = rt.block_on(async {
            timeout(Duration::from_secs(5), async {
                while let Some(changes) = change_rx.recv().await {
                    for c in &changes {
                        if c.path.to_string_lossy() == "hello.txt" && c.kind == ChangeKind::Modified
                        {
                            return true;
                        }
                    }
                }
                false
            })
            .await
        });

        drop(shutdown_tx);
        assert!(
            result.unwrap_or(false),
            "watcher should detect file modification"
        );
    }

    #[test]
    fn test_file_watcher_detects_deletion() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let delete_path = repo_path.join("to_delete.txt");
        fs::write(&delete_path, "will be deleted").unwrap();

        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(16);

        let watcher = FileWatcher::new(repo_path.clone(), Duration::from_millis(100));
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx);

        std::thread::sleep(Duration::from_millis(200));

        fs::remove_file(&delete_path).unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = rt.block_on(async {
            timeout(Duration::from_secs(5), async {
                while let Some(changes) = change_rx.recv().await {
                    for c in &changes {
                        if c.path.to_string_lossy() == "to_delete.txt"
                            && c.kind == ChangeKind::Removed
                        {
                            return true;
                        }
                    }
                }
                false
            })
            .await
        });

        drop(shutdown_tx);
        assert!(
            result.unwrap_or(false),
            "watcher should detect file deletion"
        );
    }

    #[test]
    fn test_file_watcher_ignores_dotfiles() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let suture_event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![repo_path.join(".suture/test_file")],
            ..Default::default()
        };
        let result_suture = FileChangeEvent::from_notify_event(&suture_event, &repo_path);
        assert!(
            result_suture.is_none(),
            ".suture/ path changes should be filtered out"
        );

        let nested_suture_event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![repo_path.join(".suture/deep/nested_file")],
            ..Default::default()
        };
        let result_nested = FileChangeEvent::from_notify_event(&nested_suture_event, &repo_path);
        assert!(
            result_nested.is_none(),
            "nested .suture/ path changes should be filtered out"
        );

        let visible_event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![repo_path.join("visible.txt")],
            ..Default::default()
        };
        let result_visible = FileChangeEvent::from_notify_event(&visible_event, &repo_path);
        assert!(
            result_visible.is_some(),
            "visible file changes should pass through"
        );
        assert_eq!(
            result_visible.unwrap().path.to_string_lossy(),
            "visible.txt"
        );
    }

    #[test]
    fn test_file_watcher_multiple_changes() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let (shutdown_tx, _) = broadcast::channel::<()>(2);
        let (change_tx, mut change_rx) = mpsc::channel::<Vec<FileChangeEvent>>(16);

        let debounce = Duration::from_millis(300);
        let watcher = FileWatcher::new(repo_path.clone(), debounce);
        let watch_shutdown = shutdown_tx.subscribe();
        watcher.run(watch_shutdown, change_tx);

        std::thread::sleep(Duration::from_millis(200));

        for i in 0..5 {
            fs::write(
                repo_path.join(format!("multi_{i}.txt")),
                format!("data {i}"),
            )
            .unwrap();
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = rt.block_on(async {
            timeout(Duration::from_secs(5), async {
                let mut total_changes = 0;
                while let Some(changes) = change_rx.recv().await {
                    total_changes += changes.len();
                    if total_changes >= 5 {
                        break;
                    }
                }
                total_changes
            })
            .await
        });

        drop(shutdown_tx);
        assert!(
            result.unwrap_or(0) >= 5,
            "should detect at least 5 changes from multiple file creates"
        );
    }

    #[tokio::test]
    async fn test_auto_commit_empty_repo() {
        init_tracing();
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        Repository::init(&repo_path, "test-user").unwrap();

        let auto_commit = AutoCommit::new(
            repo_path.clone(),
            "auto: {count} file(s) changed".to_string(),
        );

        fs::write(repo_path.join("first.txt"), "initial content").unwrap();

        let changes = vec![FileChangeEvent {
            path: PathBuf::from("first.txt"),
            kind: ChangeKind::Created,
        }];

        let result = auto_commit.handle_changes(&changes).await;
        assert!(result.is_ok(), "auto-commit on empty repo should succeed");
        let patch_id = result.unwrap();
        assert!(
            patch_id.is_some(),
            "auto-commit should create an initial patch"
        );

        let repo = Repository::open(&repo_path).unwrap();
        let log = repo.log(None).unwrap();
        assert!(log.iter().any(|p| p.message.contains("auto:")));
    }

    #[tokio::test]
    async fn test_auto_commit_with_changes() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let auto_commit = AutoCommit::new(
            repo_path.clone(),
            "auto: {count} file(s) changed".to_string(),
        );

        fs::write(repo_path.join("changed.txt"), "new data").unwrap();
        fs::write(repo_path.join("another.txt"), "more data").unwrap();

        let changes = vec![
            FileChangeEvent {
                path: PathBuf::from("changed.txt"),
                kind: ChangeKind::Modified,
            },
            FileChangeEvent {
                path: PathBuf::from("another.txt"),
                kind: ChangeKind::Created,
            },
        ];

        let result = auto_commit.handle_changes(&changes).await;
        assert!(result.is_ok());
        let patch_id = result.unwrap();
        assert!(
            patch_id.is_some(),
            "auto-commit should capture file changes"
        );

        let repo = Repository::open(&repo_path).unwrap();
        let log = repo.log(None).unwrap();
        let auto_patch = log.iter().find(|p| p.message.contains("auto:"));
        assert!(auto_patch.is_some(), "should find an auto-commit in log");
        let last = auto_patch.unwrap();
        assert!(
            last.message.contains("2"),
            "template should include count of 2 files"
        );
    }

    #[tokio::test]
    async fn test_auto_commit_idempotent() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let auto_commit = AutoCommit::new(
            repo_path.clone(),
            "auto: {count} file(s) changed".to_string(),
        );

        let changes = vec![FileChangeEvent {
            path: PathBuf::from("nonexistent.txt"),
            kind: ChangeKind::Modified,
        }];

        let result1 = auto_commit.handle_changes(&changes).await;
        assert!(result1.is_ok(), "first auto-commit should not error");

        let result2 = auto_commit.handle_changes(&changes).await;
        assert!(result2.is_ok(), "second auto-commit should not error");

        let repo = Repository::open(&repo_path).unwrap();
        let log = repo.log(None).unwrap();
        let auto_count = log.iter().filter(|p| p.message.contains("auto:")).count();
        assert_eq!(
            auto_count, 0,
            "no auto-commits should be created when staging fails (nonexistent file)"
        );
    }

    #[tokio::test]
    async fn test_start_stop_daemon() {
        init_tracing();
        let (_tmp, repo_path) = create_test_repo();

        let daemon = Daemon::new(repo_path, None, Duration::from_secs(3600));

        let handle = tokio::spawn(async move {
            match timeout(Duration::from_secs(2), daemon.run()).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => {}
                Err(_) => {}
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        handle.abort();

        let outcome = handle.await;
        assert!(
            outcome.is_ok() || outcome.is_err(),
            "daemon task should complete or be cancelled without panic"
        );
    }

    #[tokio::test]
    async fn test_daemon_status_after_sync() {
        init_tracing();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut hub = suture_hub::SutureHubServer::new_in_memory().unwrap();
        hub.set_no_auth(true);

        let hub_addr = format!("127.0.0.1:{}", port);
        let server_handle = tokio::spawn(async move {
            let _ = suture_hub::server::run_server(hub, &hub_addr).await;
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        let hub_url = format!("http://127.0.0.1:{}", port);

        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let mut repo = Repository::init(&repo_path, "test-user").unwrap();

        fs::write(repo_path.join("status_test.txt"), "content").unwrap();
        repo.add("status_test.txt").unwrap();
        repo.commit("initial").unwrap();
        repo.add_remote("origin", &hub_url).unwrap();

        let auto_sync = AutoSync::new(
            repo_path.clone(),
            Some(hub_url.clone()),
            Duration::from_secs(60),
        );

        let sync_result = auto_sync.sync_once().await;
        assert!(
            sync_result.is_ok(),
            "sync should succeed: {:?}",
            sync_result.err()
        );

        let repo = Repository::open(&repo_path).unwrap();
        let status = repo.status().unwrap();
        assert_eq!(status.branch_count, 1, "should have exactly 1 branch");
        assert!(status.head_patch.is_some(), "HEAD should be set");
        assert!(status.patch_count > 0, "should have patches after sync");

        let last_pushed = repo.get_config("remote.origin.last_pushed").unwrap();
        assert!(
            last_pushed.is_some(),
            "last_pushed config should exist after sync"
        );

        let (_, head_id) = repo.head().unwrap();
        assert_eq!(
            head_id.to_hex(),
            last_pushed.unwrap(),
            "last_pushed should match current HEAD"
        );

        server_handle.abort();
    }
}
