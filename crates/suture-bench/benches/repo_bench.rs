use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use suture_core::repository::Repository;
use tempfile::TempDir;

fn bench_repo_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_bench_init");

    group.bench_function("init_empty", |b| {
        b.iter_with_setup(
            || TempDir::new().unwrap(),
            |dir| {
                let _ = black_box(Repository::init(dir.path(), "bench").unwrap());
            },
        );
    });

    for n in [100usize, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("init_with_n_files", n), &n, |b, &n| {
            b.iter_with_setup(
                || {
                    let dir = TempDir::new().unwrap();
                    for i in 0..n {
                        std::fs::write(
                            dir.path().join(format!("file_{i}.txt")),
                            format!("content {i}"),
                        )
                        .unwrap();
                    }
                    dir
                },
                |dir| {
                    let _ = black_box(Repository::init(dir.path(), "bench").unwrap());
                },
            );
        });
    }

    group.finish();
}

fn bench_repo_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_bench_add");

    for n in [1usize, 10, 100] {
        group.bench_with_input(BenchmarkId::new("add_n_files", n), &n, |b, &n| {
            b.iter_with_setup(
                || {
                    let dir = TempDir::new().unwrap();
                    let repo = Repository::init(dir.path(), "bench").unwrap();
                    for i in 0..n {
                        std::fs::write(
                            dir.path().join(format!("file_{i}.txt")),
                            format!("content {i}"),
                        )
                        .unwrap();
                    }
                    (dir, repo)
                },
                |(_dir, repo)| {
                    for i in 0..n {
                        let _ = repo.add(&format!("file_{}.txt", i));
                    }
                },
            );
        });
    }

    group.finish();
}

fn bench_repo_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_bench_commit");

    group.bench_function("commit_single_file", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let repo = Repository::init(dir.path(), "bench").unwrap();
                std::fs::write(dir.path().join("file.txt"), "content").unwrap();
                (dir, repo)
            },
            |(_dir, mut repo)| {
                repo.add("file.txt").unwrap();
                let _ = black_box(repo.commit("bench commit").unwrap());
            },
        );
    });

    for n in [1usize, 10, 100] {
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
                        repo.add(&format!("file_{}.txt", i)).unwrap();
                    }
                    let _ = black_box(repo.commit(&format!("commit {} files", n)).unwrap());
                },
            );
        });
    }

    group.finish();
}

fn bench_repo_log(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_bench_log");

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
    let mut group = c.benchmark_group("repo_bench_diff");

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

fn bench_repo_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_bench_merge");

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

    group.finish();
}

criterion_group!(
    repo_bench,
    bench_repo_init,
    bench_repo_add,
    bench_repo_commit,
    bench_repo_log,
    bench_repo_diff,
    bench_repo_merge,
);
criterion_main!(repo_bench);
