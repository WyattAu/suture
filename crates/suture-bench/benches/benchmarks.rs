use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use suture_common::Hash;
use tempfile::TempDir;

use suture_core::cas::pack::PackFile;
use suture_core::cas::store::BlobStore;
use suture_core::dag::graph::PatchDag;
use suture_core::engine::apply::{apply_patch_chain, resolve_payload_to_hash};
use suture_core::engine::diff::diff_trees;
use suture_core::engine::merge::diff_lines;
use suture_core::engine::tree::FileTree;
use suture_core::patch::types::{OperationType, Patch, TouchSet};

fn bench_cas_put_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("cas_put_get");

    for size in [1024usize, 10240, 102400] {
        let data = vec![42u8; size];
        group.bench_with_input(BenchmarkId::new("put_get", size), &data, |b, data| {
            b.iter_with_setup(
                || {
                    let dir = TempDir::new().unwrap();
                    let cas = BlobStore::new(dir.path()).unwrap();
                    (dir, cas)
                },
                |(dir, cas)| {
                    let hash = black_box(cas.put_blob(data).unwrap());
                    let blob = black_box(cas.get_blob(&hash).unwrap());
                    assert_eq!(blob.len(), data.len());
                    drop(dir);
                },
            );
        });
    }
    group.finish();
}

fn bench_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("blake3_hashing");

    for size in [64usize, 1024, 10240, 102400] {
        let data = vec![42u8; size];
        group.bench_with_input(BenchmarkId::new("hash", size), &data, |b, data| {
            b.iter(|| black_box(Hash::from_data(data)));
        });
    }
    group.finish();
}

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

fn bench_dag_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_insertion");

    for n in [10usize, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("linear_chain", n), &n, |b, &n| {
            b.iter(|| {
                let mut dag = PatchDag::new();
                let mut last_id = None;
                for i in 0..n {
                    let patch = make_patch(i);
                    let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                    let id = black_box(dag.add_patch(patch, parents).unwrap());
                    last_id = Some(id);
                }
                black_box(dag);
            });
        });
    }
    group.finish();
}

fn bench_dag_lca(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_lca");

    for n in [10usize, 100, 500] {
        group.bench_with_input(BenchmarkId::new("linear_chain", n), &n, |b, &n| {
            let mut dag = PatchDag::new();
            let mut last_id = None;
            for i in 0..n {
                let patch = make_patch(i);
                let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                let id = dag.add_patch(patch, parents).unwrap();
                last_id = Some(id);
            }
            b.iter(|| {
                // Build chain each time, then LCA tip vs root
                let mut dag = PatchDag::new();
                let mut last_id = None;
                let mut first_id = None;
                for i in 0..n {
                    let patch = make_patch(i);
                    let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                    let id = dag.add_patch(patch, parents).unwrap();
                    if first_id.is_none() {
                        first_id = Some(id);
                    }
                    last_id = Some(id);
                }
                let result = black_box(dag.lca(&first_id.unwrap(), &last_id.unwrap()));
                assert!(result.is_some());
            });
        });
    }
    group.finish();
}

/// Benchmark LCA on a diamond-shaped DAG (the most common merge pattern).
/// Tests the generation-number optimization and ancestor caching.
fn bench_dag_lca_diamond(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_lca_diamond");

    for depth in [5usize, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("diamond_merge", depth),
            &depth,
            |b, &depth| {
                b.iter_with_setup(
                    || {
                        // Build a diamond DAG: root -> (left + right) -> merge, repeated `depth` times
                        let mut dag = PatchDag::new();
                        let root = make_patch(0);
                        let root_id = dag.add_patch(root, vec![]).unwrap();

                        let mut tip = root_id;
                        for d in 1..=depth {
                            let left = make_patch(d * 2);
                            let right = make_patch(d * 2 + 1);
                            let left_id = dag.add_patch(left, vec![tip]).unwrap();
                            let right_id = dag.add_patch(right, vec![tip]).unwrap();
                            let merge_p = make_patch(d * 2 + 1000);
                            let merge_id = dag.add_patch(merge_p, vec![left_id, right_id]).unwrap();
                            tip = merge_id;
                        }
                        (dag, tip)
                    },
                    |(dag, tip)| {
                        // LCA between two nodes at the same depth (both children of root)
                        // This exercises ancestor caching heavily
                        let root = dag.get_node(&dag.patch_ids()[0]).unwrap();
                        let root_id = root.patch.id;
                        let result = black_box(dag.lca(&tip, &root_id));
                        assert!(result.is_some());
                    },
                );
            },
        );
    }
    group.finish();
}

/// Benchmark ancestors() with caching — second call should be O(1).
fn bench_dag_ancestors_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_ancestors_cached");

    group.bench_function("ancestors_1k_cached", |b| {
        b.iter_with_setup(
            || {
                let mut dag = PatchDag::new();
                let mut last_id = None;
                for i in 0..1000 {
                    let patch = make_patch(i);
                    let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                    let id = dag.add_patch(patch, parents).unwrap();
                    last_id = Some(id);
                }
                dag
            },
            |dag| {
                let tip = dag.patch_ids().last().unwrap();
                // First call: BFS computation (cached)
                let _ = black_box(dag.ancestors(&tip));
                // Second call: cache hit
                let _ = black_box(dag.ancestors(&tip));
            },
        );
    });

    group.finish();
}

fn make_create_patch(i: usize) -> Patch {
    let content = format!("content_{}", i);
    let blob_hash = Hash::from_data(content.as_bytes()).to_hex();
    Patch::new(
        OperationType::Create,
        TouchSet::single(format!("addr_{}", i)),
        Some(format!("file_{}", i)),
        blob_hash.into_bytes(),
        vec![],
        "bench".to_string(),
        format!("create file {}", i),
    )
}

fn bench_patch_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("patch_chain");

    for n in [10usize, 50, 100] {
        group.bench_with_input(BenchmarkId::new("apply", n), &n, |b, &n| {
            let patches: Vec<Patch> = (0..n).map(make_create_patch).collect();
            b.iter(|| {
                let tree = black_box(apply_patch_chain(&patches, resolve_payload_to_hash).unwrap());
                assert_eq!(tree.len(), n);
            });
        });
    }
    group.finish();
}

fn bench_filetree_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("filetree_diff");

    for n in [10usize, 50, 100] {
        group.bench_with_input(BenchmarkId::new("diff_trees", n), &n, |b, &n| {
            let mut old_tree = FileTree::empty();
            let mut new_tree = FileTree::empty();
            for i in 0..n {
                let hash = Hash::from_data(format!("old_{}", i).as_bytes());
                old_tree.insert(format!("file_{}.txt", i), hash);
            }
            for i in 0..n {
                let hash = if i % 3 == 0 {
                    Hash::from_data(format!("modified_{}", i).as_bytes())
                } else {
                    Hash::from_data(format!("old_{}", i).as_bytes())
                };
                new_tree.insert(format!("file_{}.txt", i), hash);
            }
            b.iter(|| {
                let diffs = black_box(diff_trees(&old_tree, &new_tree));
                assert!(!diffs.is_empty());
            });
        });
    }
    group.finish();
}

fn bench_dag_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_large");

    group.bench_function("ancestors_10k", |b| {
        b.iter(|| {
            let mut dag = PatchDag::new();
            let mut last_id = None;
            for i in 0..10_000 {
                let patch = make_patch(i);
                let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                let id = dag.add_patch(patch, parents).unwrap();
                last_id = Some(id);
            }
            let tip = last_id.unwrap();
            black_box(dag.patch_chain(&tip));
            black_box(dag);
        });
    });

    group.finish();
}

fn bench_filetree_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("filetree_large");

    group.bench_function("insert_10k_files", |b| {
        b.iter(|| {
            let mut tree = FileTree::empty();
            for i in 0..10_000 {
                let path = format!("src/module_{:04}/file.rs", i / 100);
                tree.insert(path, Hash::from_data(format!("content {}", i).as_bytes()));
            }
            tree
        });
    });

    group.bench_function("snapshot_10k_files", |b| {
        let mut tree = FileTree::empty();
        for i in 0..10_000 {
            let path = format!("src/module_{:04}/file.rs", i / 100);
            tree.insert(path, Hash::from_data(format!("content {}", i).as_bytes()));
        }
        b.iter(|| tree.content_hash());
    });

    group.finish();
}

fn bench_diff_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_large");

    let base: Vec<String> = (0..1000)
        .map(|i| format!("line {}: original content here", i))
        .collect();
    let modified: Vec<String> = base
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i % 10 == 0 {
                format!("line {}: MODIFIED content", i)
            } else {
                line.clone()
            }
        })
        .collect();

    let base_refs: Vec<&str> = base.iter().map(|s| s.as_str()).collect();
    let mod_refs: Vec<&str> = modified.iter().map(|s| s.as_str()).collect();

    group.bench_function("patience_diff_1k_lines", |b| {
        b.iter(|| diff_lines(&base_refs, &mod_refs));
    });

    group.finish();
}

fn bench_pack_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("pack_large");

    let blobs: Vec<(Hash, Vec<u8>)> = (0..1000)
        .map(|i| {
            let data = format!(
                "blob content number {} with some padding to make it realistic",
                i
            );
            (Hash::from_data(data.as_bytes()), data.into_bytes())
        })
        .collect();

    group.bench_function("pack_create_1k_blobs", |b| {
        b.iter_with_setup(
            || {
                let dir = tempfile::tempdir().unwrap();
                let pack_dir = dir.path().join("objects").join("pack");
                (dir, pack_dir)
            },
            |(dir, pack_dir)| {
                let _ = black_box(PackFile::create(&pack_dir, &blobs));
                drop(dir);
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cas_put_get,
    bench_hashing,
    bench_dag_insertion,
    bench_dag_lca,
    bench_dag_lca_diamond,
    bench_dag_ancestors_cached,
    bench_patch_chain,
    bench_filetree_diff,
    bench_dag_large,
    bench_filetree_large,
    bench_diff_large,
    bench_pack_large,
);
criterion_main!(benches);
