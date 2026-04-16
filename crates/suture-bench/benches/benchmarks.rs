use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use suture_common::Hash;
use tempfile::TempDir;

use suture_core::cas::pack::PackFile;
use suture_core::cas::store::BlobStore;
use suture_core::dag::graph::PatchDag;
use suture_core::engine::diff::diff_trees;
use suture_core::engine::merge::diff_lines;
use suture_core::engine::tree::FileTree;
use suture_core::engine::{apply_patch_chain, resolve_payload_to_hash};
use suture_core::patch::types::{OperationType, Patch, TouchSet};
use suture_core::repository::Repository;

use suture_hub::HubStorage;
use suture_protocol::{
    apply_delta, compress as proto_compress, compute_delta, decompress as proto_decompress,
};

use suture_driver::SutureDriver;
use suture_driver_csv::CsvDriver;
use suture_driver_json::JsonDriver;
use suture_driver_toml::TomlDriver;
use suture_driver_yaml::YamlDriver;

// =============================================================================
// Existing benchmarks (preserved)
// =============================================================================

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

fn bench_dag_lca_diamond(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_lca_diamond");

    for depth in [5usize, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("diamond_merge", depth),
            &depth,
            |b, &depth| {
                b.iter_with_setup(
                    || {
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
                        let root = dag.get_node(&dag.patch_ids()[0]).unwrap();
                        let root_id = root.id();
                        let result = black_box(dag.lca(&tip, &root_id));
                        assert!(result.is_some());
                    },
                );
            },
        );
    }
    group.finish();
}

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
                let patch_ids = dag.patch_ids();
                let tip = patch_ids.last().unwrap();
                let _ = black_box(dag.ancestors(tip));
                let _ = black_box(dag.ancestors(tip));
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

// =============================================================================
// 1. Repository Operations
// =============================================================================

fn bench_repo_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_init");
    group.bench_function("init_new_repo", |b| {
        b.iter_with_setup(
            || TempDir::new().unwrap(),
            |dir| {
                let _ = black_box(Repository::init(dir.path(), "bench").unwrap());
            },
        );
    });
    group.finish();
}

fn bench_repo_add_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_add_commit");

    group.bench_function("add_commit_single_file", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let repo = Repository::init(dir.path(), "bench").unwrap();
                std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
                (dir, repo)
            },
            |(_dir, mut repo)| {
                repo.add("hello.txt").unwrap();
                black_box(repo.commit("bench commit").unwrap());
            },
        );
    });

    for n in [1usize, 10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("commit_n_files", n), &n, |b, &n| {
            b.iter_with_setup(
                || {
                    let dir = TempDir::new().unwrap();
                    let repo = Repository::init(dir.path(), "bench").unwrap();
                    for i in 0..n {
                        let path = format!("file_{}.txt", i);
                        std::fs::write(dir.path().join(&path), format!("content {}", i)).unwrap();
                    }
                    (dir, repo)
                },
                |(_dir, mut repo)| {
                    for i in 0..n {
                        let path = format!("file_{}.txt", i);
                        repo.add(&path).unwrap();
                    }
                    black_box(repo.commit(&format!("commit {} files", n)).unwrap());
                },
            );
        });
    }

    group.finish();
}

fn bench_repo_log(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_log");

    for n in [10usize, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("log_n_commits", n), &n, |b, &n| {
            b.iter_with_setup(
                || {
                    let dir = TempDir::new().unwrap();
                    let mut repo = Repository::init(dir.path(), "bench").unwrap();
                    for i in 0..n {
                        let path = format!("file_{}.txt", i % 10);
                        std::fs::write(dir.path().join(&path), format!("content revision {}", i))
                            .unwrap();
                        repo.add(&path).unwrap();
                        repo.commit(&format!("commit {}", i)).unwrap();
                    }
                    (dir, repo)
                },
                |(_dir, repo)| {
                    let _ = black_box(repo.log(None).unwrap());
                },
            );
        });
    }

    group.finish();
}

fn bench_repo_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_diff");

    for n in [10usize, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("diff_n_files_changed", n), &n, |b, &n| {
            b.iter_with_setup(
                || {
                    let dir = TempDir::new().unwrap();
                    let mut repo = Repository::init(dir.path(), "bench").unwrap();
                    for i in 0..n {
                        let path = format!("file_{}.txt", i);
                        std::fs::write(dir.path().join(&path), format!("original {}", i)).unwrap();
                        repo.add(&path).unwrap();
                    }
                    repo.commit("initial").unwrap();
                    for i in 0..n {
                        let path = format!("file_{}.txt", i);
                        std::fs::write(dir.path().join(&path), format!("modified {}", i)).unwrap();
                        repo.add(&path).unwrap();
                    }
                    repo.commit("changes").unwrap();
                    (dir, repo)
                },
                |(_dir, repo)| {
                    let _ = black_box(repo.diff(Some("HEAD~1"), Some("HEAD")).unwrap());
                },
            );
        });
    }

    group.finish();
}

fn bench_repo_branch(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_branch");

    group.bench_function("create_branch", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let mut repo = Repository::init(dir.path(), "bench").unwrap();
                std::fs::write(dir.path().join("f.txt"), "data").unwrap();
                repo.add("f.txt").unwrap();
                repo.commit("initial").unwrap();
                (dir, repo)
            },
            |(_dir, mut repo)| {
                repo.create_branch("feature", None).unwrap();
            },
        );
    });

    group.bench_function("checkout_branch", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let mut repo = Repository::init(dir.path(), "bench").unwrap();
                std::fs::write(dir.path().join("main.txt"), "main data").unwrap();
                repo.add("main.txt").unwrap();
                repo.commit("main commit").unwrap();
                repo.create_branch("feature", None).unwrap();
                (dir, repo)
            },
            |(_dir, mut repo)| {
                black_box(repo.checkout("feature").unwrap());
                black_box(repo.checkout("main").unwrap());
            },
        );
    });

    group.finish();
}

fn bench_repo_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_merge");

    group.bench_function("merge_clean", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let mut repo = Repository::init(dir.path(), "bench").unwrap();
                std::fs::write(dir.path().join("shared.txt"), "base").unwrap();
                std::fs::write(dir.path().join("a.txt"), "a_original").unwrap();
                std::fs::write(dir.path().join("b.txt"), "b_original").unwrap();
                repo.add("shared.txt").unwrap();
                repo.add("a.txt").unwrap();
                repo.add("b.txt").unwrap();
                repo.commit("base").unwrap();

                repo.create_branch("feature", None).unwrap();
                repo.checkout("feature").unwrap();
                std::fs::write(dir.path().join("b.txt"), "b_modified").unwrap();
                repo.add("b.txt").unwrap();
                repo.commit("modify b on feature").unwrap();
                repo.checkout("main").unwrap();
                std::fs::write(dir.path().join("a.txt"), "a_modified").unwrap();
                repo.add("a.txt").unwrap();
                repo.commit("modify a on main").unwrap();

                (dir, repo)
            },
            |(_dir, mut repo)| {
                let result = black_box(repo.execute_merge("feature").unwrap());
                assert!(result.is_clean);
            },
        );
    });

    group.bench_function("merge_conflicting", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let mut repo = Repository::init(dir.path(), "bench").unwrap();
                std::fs::write(dir.path().join("conflict.txt"), "base_content").unwrap();
                repo.add("conflict.txt").unwrap();
                repo.commit("base").unwrap();

                repo.create_branch("feature", None).unwrap();
                repo.checkout("feature").unwrap();
                std::fs::write(dir.path().join("conflict.txt"), "ours_content").unwrap();
                repo.add("conflict.txt").unwrap();
                repo.commit("ours change").unwrap();
                repo.checkout("main").unwrap();
                std::fs::write(dir.path().join("conflict.txt"), "theirs_content").unwrap();
                repo.add("conflict.txt").unwrap();
                repo.commit("theirs change").unwrap();

                (dir, repo)
            },
            |(_dir, mut repo)| {
                let result = black_box(repo.execute_merge("feature").unwrap());
                assert!(!result.is_clean);
            },
        );
    });

    group.finish();
}

// =============================================================================
// 2. Semantic Merge
// =============================================================================

fn generate_json_keys(n: usize) -> String {
    let mut map = serde_json::Map::new();
    for i in 0..n {
        map.insert(
            format!("key_{}", i),
            serde_json::Value::String(format!("value_{}", i)),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap()
}

fn generate_yaml_keys(n: usize) -> String {
    let mut lines = String::new();
    for i in 0..n {
        lines.push_str(&format!("key_{}: value_{}\n", i, i));
    }
    lines
}

fn generate_toml_keys(n: usize) -> String {
    let mut lines = String::new();
    for i in 0..n {
        lines.push_str(&format!("key_{} = \"value_{}\"\n", i, i));
    }
    lines
}

fn generate_csv_rows(n: usize) -> String {
    let mut csv = String::from("id,name,value\n");
    for i in 0..n {
        csv.push_str(&format!("{},item_{},{}\n", i, i, i * 10));
    }
    csv
}

fn bench_semantic_merge_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic_merge_json");
    let driver = JsonDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_json_keys(size);
        let mut ours_map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&base).unwrap();
        ours_map.insert(
            format!("key_{}", size / 2),
            serde_json::Value::String("modified_by_ours".into()),
        );
        let ours = serde_json::to_string(&serde_json::Value::Object(ours_map)).unwrap();

        let mut theirs_map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&base).unwrap();
        theirs_map.insert(
            format!("key_{}", size / 3),
            serde_json::Value::String("modified_by_theirs".into()),
        );
        theirs_map.insert(
            format!("new_key_{}", size),
            serde_json::Value::String("new_value".into()),
        );
        let theirs = serde_json::to_string(&serde_json::Value::Object(theirs_map)).unwrap();

        group.bench_with_input(BenchmarkId::new("merge", size), &size, |b, _| {
            b.iter(|| {
                let _ = black_box(driver.merge(&base, &ours, &theirs).unwrap());
            });
        });
    }
    group.finish();
}

fn bench_semantic_merge_yaml(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic_merge_yaml");
    let driver = YamlDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_yaml_keys(size);
        let ours = base.replacen(
            &format!("key_{}: value_{}", size / 2, size / 2),
            &format!("key_{}: ours_value", size / 2),
            1,
        );
        let theirs = base.replacen(
            &format!("key_{}: value_{}", size / 3, size / 3),
            &format!("key_{}: theirs_value", size / 3),
            1,
        );

        group.bench_with_input(BenchmarkId::new("merge", size), &size, |b, _| {
            b.iter(|| {
                let _ = black_box(driver.merge(&base, &ours, &theirs).unwrap());
            });
        });
    }
    group.finish();
}

fn bench_semantic_merge_toml(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic_merge_toml");
    let driver = TomlDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_toml_keys(size);
        let ours = base.replacen(
            &format!("key_{} = \"value_{}\"", size / 2, size / 2),
            &format!("key_{} = \"ours_value\"", size / 2),
            1,
        );
        let theirs = base.replacen(
            &format!("key_{} = \"value_{}\"", size / 3, size / 3),
            &format!("key_{} = \"theirs_value\"", size / 3),
            1,
        );

        group.bench_with_input(BenchmarkId::new("merge", size), &size, |b, _| {
            b.iter(|| {
                let _ = black_box(driver.merge(&base, &ours, &theirs).unwrap());
            });
        });
    }
    group.finish();
}

fn bench_semantic_merge_csv(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic_merge_csv");
    let driver = CsvDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_csv_rows(size);
        let ours = base.replacen("item_0", "MODIFIED_ITEM", 1);
        let theirs = format!("id,name,value\n999,extra_row,9999\n{}", base);

        group.bench_with_input(BenchmarkId::new("merge", size), &size, |b, _| {
            b.iter(|| {
                let _ = black_box(driver.merge(&base, &ours, &theirs).unwrap());
            });
        });
    }
    group.finish();
}

// =============================================================================
// 3. Protocol Operations
// =============================================================================

fn make_data_with_suffix(size: usize, suffix: &str) -> Vec<u8> {
    let base: Vec<u8> = (0..size.saturating_sub(suffix.len()))
        .map(|i| (i % 251) as u8)
        .collect();
    let mut data = base;
    data.extend_from_slice(suffix.as_bytes());
    data
}

fn bench_delta_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("delta_compute");

    let sizes = [
        ("100B", 100usize),
        ("10KB", 10_240usize),
        ("1MB", 1_048_576usize),
    ];

    for (label, size) in sizes {
        let base = make_data_with_suffix(size, "BASE_END");
        let target = make_data_with_suffix(size, "CHANGED_END");

        group.bench_with_input(BenchmarkId::new(label, size), &size, |b, _| {
            b.iter(|| {
                let (_base_copy, delta) = black_box(compute_delta(&base, &target));
                black_box(delta);
            });
        });
    }
    group.finish();
}

fn bench_delta_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("delta_apply");

    let sizes = [
        ("100B", 100usize),
        ("10KB", 10_240usize),
        ("1MB", 1_048_576usize),
    ];

    for (label, size) in sizes {
        let base = make_data_with_suffix(size, "BASE_END");
        let target = make_data_with_suffix(size, "CHANGED_END");
        let (_base_copy, delta) = compute_delta(&base, &target);

        group.bench_with_input(BenchmarkId::new(label, size), &size, |b, _| {
            b.iter(|| {
                let result = black_box(apply_delta(&base, &delta));
                assert_eq!(result, target);
            });
        });
    }
    group.finish();
}

fn bench_compress_decompress(c: &mut Criterion) {
    let mut group = c.benchmark_group("compress_decompress");

    let sizes = [
        ("100B", 100usize),
        ("10KB", 10_240usize),
        ("1MB", 1_048_576usize),
    ];

    for (label, size) in sizes {
        let data = make_data_with_suffix(size, "COMPRESS_ME");

        group.bench_function(format!("compress_{}", label), |b| {
            b.iter(|| {
                let compressed = black_box(proto_compress(&data).unwrap());
                black_box(compressed);
            });
        });

        let compressed = proto_compress(&data).unwrap();
        group.bench_function(format!("decompress_{}", label), |b| {
            b.iter(|| {
                let result = black_box(proto_decompress(&compressed).unwrap());
                black_box(result);
            });
        });
    }
    group.finish();
}

// =============================================================================
// 4. Hub Operations
// =============================================================================

fn bench_hub_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("hub_storage");

    for n in [10usize, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("push_n_patches_blobs", n), &n, |b, &n| {
            b.iter_with_setup(
                || HubStorage::open_in_memory().unwrap(),
                |store| {
                    store.ensure_repo("bench-repo").unwrap();
                    for i in 0..n {
                        let id_hex = Hash::from_data(format!("patch_{}", i).as_bytes()).to_hex();
                        let patch = suture_protocol::PatchProto {
                            id: suture_protocol::HashProto {
                                value: id_hex.clone(),
                            },
                            operation_type: "Create".to_string(),
                            touch_set: vec![format!("file_{}", i)],
                            target_path: Some(format!("file_{}", i)),
                            payload: String::new(),
                            parent_ids: vec![],
                            author: "bench".to_string(),
                            message: format!("patch {}", i),
                            timestamp: i as u64,
                        };
                        store.insert_patch("bench-repo", &patch).unwrap();
                    }
                    for i in 0..n {
                        let hash_hex = Hash::from_data(format!("blob_{}", i).as_bytes()).to_hex();
                        let data = format!("blob content {}", i);
                        store
                            .store_blob("bench-repo", &hash_hex, data.as_bytes())
                            .unwrap();
                    }
                },
            );
        });

        group.bench_with_input(BenchmarkId::new("pull_n_patches_blobs", n), &n, |b, &n| {
            b.iter_with_setup(
                || {
                    let store = HubStorage::open_in_memory().unwrap();
                    store.ensure_repo("bench-repo").unwrap();
                    for i in 0..n {
                        let id_hex = Hash::from_data(format!("patch_{}", i).as_bytes()).to_hex();
                        let patch = suture_protocol::PatchProto {
                            id: suture_protocol::HashProto { value: id_hex },
                            operation_type: "Create".to_string(),
                            touch_set: vec![format!("file_{}", i)],
                            target_path: Some(format!("file_{}", i)),
                            payload: String::new(),
                            parent_ids: vec![],
                            author: "bench".to_string(),
                            message: format!("patch {}", i),
                            timestamp: i as u64,
                        };
                        store.insert_patch("bench-repo", &patch).unwrap();
                    }
                    for i in 0..n {
                        let hash_hex = Hash::from_data(format!("blob_{}", i).as_bytes()).to_hex();
                        let data = format!("blob content {}", i);
                        store
                            .store_blob("bench-repo", &hash_hex, data.as_bytes())
                            .unwrap();
                    }
                    store
                },
                |store| {
                    let patches = black_box(store.get_all_patches("bench-repo").unwrap());
                    assert_eq!(patches.len(), n);
                    let blobs = black_box(store.get_all_blobs("bench-repo").unwrap());
                    assert_eq!(blobs.len(), n);
                },
            );
        });
    }

    group.bench_function("push_pull_roundtrip_100", |b| {
        b.iter_with_setup(
            || HubStorage::open_in_memory().unwrap(),
            |store| {
                store.ensure_repo("bench-repo").unwrap();
                for i in 0..100 {
                    let id_hex = Hash::from_data(format!("patch_{}", i).as_bytes()).to_hex();
                    let patch = suture_protocol::PatchProto {
                        id: suture_protocol::HashProto { value: id_hex },
                        operation_type: "Create".to_string(),
                        touch_set: vec![format!("file_{}", i)],
                        target_path: Some(format!("file_{}", i)),
                        payload: String::new(),
                        parent_ids: vec![],
                        author: "bench".to_string(),
                        message: format!("patch {}", i),
                        timestamp: i as u64,
                    };
                    store.insert_patch("bench-repo", &patch).unwrap();
                    let hash_hex = Hash::from_data(format!("blob_{}", i).as_bytes()).to_hex();
                    let data = format!("blob content {}", i);
                    store
                        .store_blob("bench-repo", &hash_hex, data.as_bytes())
                        .unwrap();
                }
                let patches = black_box(store.get_all_patches("bench-repo").unwrap());
                let blobs = black_box(store.get_all_blobs("bench-repo").unwrap());
                assert_eq!(patches.len(), 100);
                assert_eq!(blobs.len(), 100);
            },
        );
    });

    group.finish();
}

// =============================================================================
// 5. Large File Handling
// =============================================================================

fn bench_large_json_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_json_commit");

    let sizes = [
        ("10KB", 10_240usize),
        ("100KB", 102_400usize),
        ("1MB", 1_048_576usize),
    ];

    for (label, target_size) in sizes {
        let json_content = generate_json_keys(target_size / 30);

        group.bench_with_input(
            BenchmarkId::new(label, json_content.len()),
            &json_content,
            |b, content| {
                b.iter_with_setup(
                    || {
                        let dir = TempDir::new().unwrap();
                        let repo = Repository::init(dir.path(), "bench").unwrap();
                        std::fs::write(dir.path().join("data.json"), content).unwrap();
                        (dir, repo)
                    },
                    |(_dir, mut repo)| {
                        repo.add("data.json").unwrap();
                        black_box(repo.commit(&format!("commit {} json", label)).unwrap());
                    },
                );
            },
        );
    }
    group.finish();
}

fn bench_large_yaml_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_yaml_commit");

    let sizes = [("10KB", 10_240usize), ("100KB", 102_400usize)];

    for (label, target_size) in sizes {
        let yaml_content = generate_yaml_keys(target_size / 20);

        group.bench_with_input(
            BenchmarkId::new(label, yaml_content.len()),
            &yaml_content,
            |b, content| {
                b.iter_with_setup(
                    || {
                        let dir = TempDir::new().unwrap();
                        let repo = Repository::init(dir.path(), "bench").unwrap();
                        std::fs::write(dir.path().join("data.yaml"), content).unwrap();
                        (dir, repo)
                    },
                    |(_dir, mut repo)| {
                        repo.add("data.yaml").unwrap();
                        black_box(repo.commit(&format!("commit {} yaml", label)).unwrap());
                    },
                );
            },
        );
    }
    group.finish();
}

// =============================================================================
// Criterion groups
// =============================================================================

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
    bench_repo_init,
    bench_repo_add_commit,
    bench_repo_log,
    bench_repo_diff,
    bench_repo_branch,
    bench_repo_merge,
    bench_semantic_merge_json,
    bench_semantic_merge_yaml,
    bench_semantic_merge_toml,
    bench_semantic_merge_csv,
    bench_delta_compute,
    bench_delta_apply,
    bench_compress_decompress,
    bench_hub_storage,
    bench_large_json_commit,
    bench_large_yaml_commit,
);
criterion_main!(benches);
