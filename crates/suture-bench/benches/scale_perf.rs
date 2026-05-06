//! Scale performance benchmarks for Suture.
//!
//! Tests behavior under production-scale loads:
//! - 10K+ patch DAGs (ancestors, LCA, patch_chain, merges)
//! - Deep branch histories (100+ branches, fan-out/fan-in)
//! - 100+ repositories in hub storage
//! - 10K+ file trees
//! - Large diffs (10K+ lines)
//! - Multi-threaded concurrent access (read/write)

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use suture_common::Hash;
use suture_core::cas::store::BlobStore;
use suture_core::dag::graph::PatchDag;
use suture_core::engine::merge::diff_lines;
use suture_core::engine::{diff_trees, tree::FileTree};
use suture_core::patch::types::{OperationType, Patch, TouchSet};
use suture_core::repository::Repository;
use suture_hub::HubStorage;
use suture_protocol::{HashProto, PatchProto};
use tempfile::TempDir;

// =============================================================================
// Helpers
// =============================================================================
fn make_patch(i: usize) -> Patch {
    Patch::new(
        OperationType::Modify,
        TouchSet::single(format!("addr_{}", i)),
        Some(format!("file_{}", i)),
        vec![],
        vec![],
        "bench".to_string(),
        format!("patch {}", i),
    )
}

fn make_hub_patch(i: usize, parent_hex: Option<&str>) -> PatchProto {
    let id_hex = Hash::from_data(format!("scale_patch_{}", i).as_bytes()).to_hex();
    PatchProto {
        id: HashProto { value: id_hex },
        operation_type: "Create".to_string(),
        touch_set: vec![format!("file_{}", i)],
        target_path: Some(format!("file_{}", i)),
        payload: String::new(),
        parent_ids: parent_hex
            .map(|h| {
                vec![HashProto {
                    value: h.to_string(),
                }]
            })
            .unwrap_or_default(),
        author: "bench".to_string(),
        message: format!("patch {}", i),
        timestamp: i as u64,
    }
}

// =============================================================================
// 1. DAG at 10K+ scale
// =============================================================================

fn bench_dag_10k_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_dag_insertion");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("insert_10k_linear", |b| {
        b.iter(|| {
            let mut dag = PatchDag::new();
            let mut last_id = None;
            for i in 0..10_000 {
                let patch = make_patch(i);
                let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                let id = dag.add_patch(patch, parents).unwrap();
                last_id = Some(id);
            }
            black_box(dag)
        });
    });

    group.finish();
}

fn bench_dag_10k_ancestors(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_dag_ancestors");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    // Build a 10K-node linear chain once, then benchmark ancestors
    let mut dag = PatchDag::new();
    let mut last_id = None;
    for i in 0..10_000 {
        let patch = make_patch(i);
        let parents = last_id.map(|id| vec![id]).unwrap_or_default();
        let id = dag.add_patch(patch, parents).unwrap();
        last_id = Some(id);
    }
    let tip = last_id.unwrap();

    group.bench_function("ancestors_10k_linear", |b| {
        b.iter(|| {
            let result = black_box(dag.ancestors(&tip));
            assert_eq!(result.len(), 9_999);
        });
    });

    group.finish();
}

fn bench_dag_10k_patch_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_dag_patch_chain");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    let mut dag = PatchDag::new();
    let mut last_id = None;
    for i in 0..10_000 {
        let patch = make_patch(i);
        let parents = last_id.map(|id| vec![id]).unwrap_or_default();
        let id = dag.add_patch(patch, parents).unwrap();
        last_id = Some(id);
    }
    let tip = last_id.unwrap();

    group.bench_function("patch_chain_10k", |b| {
        b.iter(|| {
            let chain = black_box(dag.patch_chain(&tip));
            assert_eq!(chain.len(), 10_000);
        });
    });

    group.finish();
}

fn bench_dag_10k_lca(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_dag_lca");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    // Build diamond: root -> 5K left -> merge -> 5K right -> merge_tip
    let mut dag = PatchDag::new();
    let root = make_patch(0);
    let root_id = dag.add_patch(root, vec![]).unwrap();

    let mut left_id = root_id;
    for i in 1..5_000 {
        let patch = make_patch(i);
        left_id = dag.add_patch(patch, vec![left_id]).unwrap();
    }

    let mut right_id = root_id;
    for i in 5_000..10_000 {
        let patch = make_patch(i);
        right_id = dag.add_patch(patch, vec![right_id]).unwrap();
    }

    let merge_patch = make_patch(10_000);
    let merge_id = dag.add_patch(merge_patch, vec![left_id, right_id]).unwrap();

    group.bench_function("lca_5k_branches", |b| {
        b.iter(|| {
            let result = black_box(dag.lca(&left_id, &right_id));
            assert!(result.is_some());
        });
    });

    group.bench_function("lca_tip_vs_root_10k", |b| {
        b.iter(|| {
            let result = black_box(dag.lca(&merge_id, &root_id));
            assert!(result.is_some());
        });
    });

    group.finish();
}

// =============================================================================
// 2. Deep branch histories
// =============================================================================

fn build_fanout_dag(
    branches: usize,
    depth_per_branch: usize,
) -> (PatchDag, Vec<suture_common::Hash>) {
    let mut dag = PatchDag::new();
    let root = make_patch(0);
    let root_id = dag.add_patch(root, vec![]).unwrap();

    let mut branch_tips = Vec::new();
    for b in 0..branches {
        let mut tip = root_id;
        for d in 0..depth_per_branch {
            let idx = b * depth_per_branch + d + 1;
            let patch = make_patch(idx);
            tip = dag.add_patch(patch, vec![tip]).unwrap();
        }
        branch_tips.push(tip);
    }

    // Merge all branches back
    let mut merge_tip = branch_tips[0];
    for i in 1..branch_tips.len() {
        let merge_patch = make_patch(branches * depth_per_branch + i + 1);
        merge_tip = dag
            .add_patch(merge_patch, vec![merge_tip, branch_tips[i]])
            .unwrap();
    }

    (dag, branch_tips)
}

fn bench_dag_fanout(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_dag_fanout");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    // 100 branches, 50 patches each = 5K nodes + 99 merge nodes
    group.bench_function("build_100_branches_50_deep", |b| {
        b.iter(|| {
            let (dag, tips) = build_fanout_dag(100, 50);
            assert_eq!(tips.len(), 100);
            black_box(dag);
        });
    });

    // LCA across branches
    let (dag, tips) = build_fanout_dag(100, 50);

    group.bench_function("lca_cross_branch_100", |b| {
        b.iter(|| {
            // LCA between branch 0 tip and branch 99 tip should be root
            let result = black_box(dag.lca(&tips[0], &tips[99]));
            assert!(result.is_some());
        });
    });

    group.finish();
}

fn bench_dag_wide_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_dag_wide_merge");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    // Octopus merge: 10 branches merged into one
    let (mut dag, tips) = build_fanout_dag(10, 100);

    group.bench_function("octopus_10_parents", |b| {
        b.iter(|| {
            let merge_patch = make_patch(999_999);
            let _ = black_box(dag.add_patch(merge_patch, tips.clone()).unwrap());
        });
    });

    group.finish();
}

// =============================================================================
// 3. Hub storage at scale
// =============================================================================

fn bench_hub_1000_repos(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_hub_repos");
    group.sample_size(10);

    group.bench_function("create_1000_repos", |b| {
        b.iter_with_setup(
            || HubStorage::open_in_memory().unwrap(),
            |store| {
                for i in 0..1_000 {
                    let name = format!("scale-repo-{}", i);
                    black_box(store.ensure_repo(&name).unwrap());
                }
            },
        );
    });

    // List 1000 repos
    group.bench_function("list_1000_repos", |b| {
        b.iter_with_setup(
            || {
                let store = HubStorage::open_in_memory().unwrap();
                for i in 0..1_000 {
                    let name = format!("scale-repo-{}", i);
                    store.ensure_repo(&name).unwrap();
                }
                store
            },
            |store| {
                let repos = black_box(store.list_repos().unwrap());
                assert_eq!(repos.len(), 1_000);
            },
        );
    });

    group.finish();
}

fn bench_hub_10k_patches(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_hub_patches");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("push_10k_patches", |b| {
        b.iter_with_setup(
            || HubStorage::open_in_memory().unwrap(),
            |store| {
                store.ensure_repo("scale-repo").unwrap();
                for i in 0..10_000 {
                    let parent = if i > 0 {
                        Some(Hash::from_data(format!("scale_patch_{}", i - 1).as_bytes()).to_hex())
                    } else {
                        None
                    };
                    let patch = make_hub_patch(i, parent.as_deref());
                    store.insert_patch("scale-repo", &patch).unwrap();
                }
            },
        );
    });

    group.bench_function("pull_10k_patches", |b| {
        b.iter_with_setup(
            || {
                let store = HubStorage::open_in_memory().unwrap();
                store.ensure_repo("scale-repo").unwrap();
                for i in 0..10_000 {
                    let parent = if i > 0 {
                        Some(Hash::from_data(format!("scale_patch_{}", i - 1).as_bytes()).to_hex())
                    } else {
                        None
                    };
                    let patch = make_hub_patch(i, parent.as_deref());
                    store.insert_patch("scale-repo", &patch).unwrap();
                }
                store
            },
            |store| {
                let patches = black_box(store.get_all_patches_unbounded("scale-repo").unwrap());
                assert_eq!(patches.len(), 10_000);
            },
        );
    });

    group.finish();
}

// =============================================================================
// 4. File tree at scale
// =============================================================================

fn bench_filetree_100k(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_filetree");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("insert_100k_files", |b| {
        b.iter(|| {
            let mut tree = FileTree::empty();
            for i in 0..100_000 {
                let dir = i / 1000;
                let file = i % 1000;
                let path = format!("src/module_{:04}/file_{:04}.rs", dir, file);
                tree.insert(path, Hash::from_data(format!("content {}", i).as_bytes()));
            }
            black_box(tree);
        });
    });

    group.bench_function("snapshot_100k_files", |b| {
        let mut tree = FileTree::empty();
        for i in 0..100_000 {
            let dir = i / 1000;
            let file = i % 1000;
            let path = format!("src/module_{:04}/file_{:04}.rs", dir, file);
            tree.insert(path, Hash::from_data(format!("content {}", i).as_bytes()));
        }
        b.iter(|| black_box(tree.content_hash()));
    });

    group.finish();
}

fn bench_filetree_diff_10k(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_filetree_diff");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    let mut old_tree = FileTree::empty();
    let mut new_tree = FileTree::empty();
    for i in 0..10_000 {
        let path = format!("config/setting_{:04}.json", i);
        let hash = Hash::from_data(format!("old_{}", i).as_bytes());
        old_tree.insert(path.clone(), hash);
    }
    // Modify 10% of files, add 100 new, remove 50
    for i in 0..10_000 {
        let path = format!("config/setting_{:04}.json", i);
        let hash = if i % 10 == 0 {
            Hash::from_data(format!("modified_{}", i).as_bytes())
        } else {
            Hash::from_data(format!("old_{}", i).as_bytes())
        };
        new_tree.insert(path, hash);
    }
    for i in 10_000..10_100 {
        let path = format!("config/new_setting_{:04}.json", i);
        let hash = Hash::from_data(format!("new_{}", i).as_bytes());
        new_tree.insert(path, hash);
    }

    group.bench_function("diff_10k_files_10pct_changed", |b| {
        b.iter(|| {
            let diffs = black_box(diff_trees(&old_tree, &new_tree));
            assert!(!diffs.is_empty());
        });
    });

    group.finish();
}

// =============================================================================
// 5. Large diffs
// =============================================================================

fn bench_diff_10k_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_diff");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    let base: Vec<String> = (0..10_000)
        .map(|i| {
            format!(
                "line {}: original content here with some padding to make it realistic",
                i
            )
        })
        .collect();

    // 1% changes
    let modified_1pct: Vec<String> = base
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i % 100 == 0 {
                format!("line {}: MODIFIED content here", i)
            } else {
                line.clone()
            }
        })
        .collect();

    // 10% changes
    let modified_10pct: Vec<String> = base
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i % 10 == 0 {
                format!("line {}: MODIFIED content here", i)
            } else {
                line.clone()
            }
        })
        .collect();

    // 50% changes
    let modified_50pct: Vec<String> = base
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i % 2 == 0 {
                format!("line {}: MODIFIED content here", i)
            } else {
                line.clone()
            }
        })
        .collect();

    let base_refs: Vec<&str> = base.iter().map(|s| s.as_str()).collect();

    for (label, modified) in [
        ("1pct_change", &modified_1pct),
        ("10pct_change", &modified_10pct),
        ("50pct_change", &modified_50pct),
    ] {
        let mod_refs: Vec<&str> = modified.iter().map(|s| s.as_str()).collect();
        group.bench_function(format!("patience_diff_10k_lines_{}", label), |b| {
            b.iter(|| diff_lines(&base_refs, &mod_refs));
        });
    }

    group.finish();
}

// =============================================================================
// 6. CAS at scale
// =============================================================================

fn bench_cas_10k_blobs(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_cas");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("store_10k_blobs_1kb", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let cas = BlobStore::new_uncompressed(dir.path()).unwrap();
                (dir, cas)
            },
            |(dir, cas)| {
                for i in 0..10_000 {
                    let data = format!("blob content number {} with padding", i);
                    black_box(cas.put_blob(data.as_bytes()).unwrap());
                }
                drop(dir);
            },
        );
    });

    group.bench_function("lookup_10k_blobs_random", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let cas = BlobStore::new_uncompressed(dir.path()).unwrap();
                let mut hashes = Vec::with_capacity(10_000);
                for i in 0..10_000 {
                    let data = format!("blob content number {} with padding", i);
                    let hash = cas.put_blob(data.as_bytes()).unwrap();
                    hashes.push(hash);
                }
                (dir, cas, hashes)
            },
            |(dir, cas, hashes)| {
                // Random-access pattern: read every 7th blob
                for i in 0..1_000 {
                    let idx = (i * 7) % 10_000;
                    let blob = black_box(cas.get_blob(&hashes[idx]).unwrap());
                    assert!(!blob.is_empty());
                }
                drop(dir);
            },
        );
    });

    group.finish();
}

// =============================================================================
// 7. Multi-threaded concurrent access
// =============================================================================

fn bench_concurrent_repo_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_concurrent");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    // Build a repo with 100 commits
    let dir = TempDir::new().unwrap();
    let mut repo = Repository::init(dir.path(), "bench").unwrap();
    for i in 0..100 {
        let path = format!("file_{}.txt", i % 10);
        std::fs::write(dir.path().join(&path), format!("content revision {}", i)).unwrap();
        repo.add(&path).unwrap();
        repo.commit(&format!("commit {}", i)).unwrap();
    }
    let repo_path = dir.path().to_path_buf();
    drop(repo);
    drop(dir);

    group.bench_function("4_threads_log_100", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let path = repo_path.clone();
                    std::thread::spawn(move || {
                        let repo = Repository::open(&path).unwrap();
                        let log = repo.log(None).unwrap();
                        black_box(log.len());
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

fn bench_concurrent_hub_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_concurrent_hub");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("4_threads_push_100_patches_each", |b| {
        b.iter_with_setup(
            || {
                let store = Arc::new(HubStorage::open_in_memory().unwrap());
                store.ensure_repo("concurrent-repo").unwrap();
                store
            },
            |store| {
                let handles: Vec<_> = (0..4)
                    .map(|thread_id| {
                        let store = Arc::clone(&store);
                        std::thread::spawn(move || {
                            for i in 0..100 {
                                let global_id = thread_id * 100 + i;
                                let id_hex =
                                    Hash::from_data(format!("concurrent_{}", global_id).as_bytes())
                                        .to_hex();
                                let patch = PatchProto {
                                    id: HashProto { value: id_hex },
                                    operation_type: "Create".to_string(),
                                    touch_set: vec![format!("file_{}", global_id)],
                                    target_path: Some(format!("file_{}", global_id)),
                                    payload: String::new(),
                                    parent_ids: vec![],
                                    author: format!("thread_{}", thread_id),
                                    message: format!("patch {}", global_id),
                                    timestamp: global_id as u64,
                                };
                                store.insert_patch("concurrent-repo", &patch).unwrap();
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
                let patches = store.get_all_patches_unbounded("concurrent-repo").unwrap();
                assert_eq!(patches.len(), 400);
            },
        );
    });

    group.finish();
}

fn bench_concurrent_cas_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_concurrent_cas");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("4_threads_store_1k_blobs_each", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let cas = Arc::new(BlobStore::new_uncompressed(dir.path()).unwrap());
                (dir, cas)
            },
            |(dir, cas)| {
                let handles: Vec<_> = (0..4)
                    .map(|thread_id| {
                        let cas = Arc::clone(&cas);
                        std::thread::spawn(move || {
                            for i in 0..1_000 {
                                let data =
                                    format!("thread {} blob {} with padding data", thread_id, i);
                                let hash = cas.put_blob(data.as_bytes()).unwrap();
                                black_box(hash);
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
                drop(dir);
            },
        );
    });

    group.finish();
}

// =============================================================================
// 8. Repository operations at scale
// =============================================================================

fn bench_repo_10k_commits(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_repo");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(60));

    group.bench_function("create_10k_commits", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let repo = Repository::init(dir.path(), "bench").unwrap();
                (dir, repo)
            },
            |(dir, mut repo)| {
                for i in 0..10_000 {
                    let path = format!("file_{}.txt", i % 10);
                    std::fs::write(dir.path().join(&path), format!("content revision {}", i))
                        .unwrap();
                    repo.add(&path).unwrap();
                    repo.commit(&format!("commit {}", i)).unwrap();
                }
            },
        );
    });

    group.bench_function("log_10k_commits", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let mut repo = Repository::init(dir.path(), "bench").unwrap();
                for i in 0..10_000 {
                    let path = format!("file_{}.txt", i % 10);
                    std::fs::write(dir.path().join(&path), format!("content revision {}", i))
                        .unwrap();
                    repo.add(&path).unwrap();
                    repo.commit(&format!("commit {}", i)).unwrap();
                }
                (dir, repo)
            },
            |(_dir, repo)| {
                let log = black_box(repo.log(None).unwrap());
                assert_eq!(log.len(), 10_000);
            },
        );
    });

    group.finish();
}

fn bench_repo_1k_files_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale_repo_files");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("add_commit_1000_files", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let repo = Repository::init(dir.path(), "bench").unwrap();
                for i in 0..1_000 {
                    let path = format!("file_{:04}.txt", i);
                    std::fs::write(dir.path().join(&path), format!("content {}", i)).unwrap();
                }
                (dir, repo)
            },
            |(_dir, mut repo)| {
                for i in 0..1_000 {
                    repo.add(&format!("file_{:04}.txt", i)).unwrap();
                }
                black_box(repo.commit("commit 1000 files").unwrap());
            },
        );
    });

    group.finish();
}

// =============================================================================
// Criterion groups
// =============================================================================

criterion_group!(
    scale_benches,
    bench_dag_10k_insertion,
    bench_dag_10k_ancestors,
    bench_dag_10k_patch_chain,
    bench_dag_10k_lca,
    bench_dag_fanout,
    bench_dag_wide_merge,
    bench_hub_1000_repos,
    bench_hub_10k_patches,
    bench_filetree_100k,
    bench_filetree_diff_10k,
    bench_diff_10k_lines,
    bench_cas_10k_blobs,
    bench_concurrent_repo_reads,
    bench_concurrent_hub_writes,
    bench_concurrent_cas_writes,
    bench_repo_10k_commits,
    bench_repo_1k_files_commit,
);
criterion_main!(scale_benches);
