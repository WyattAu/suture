use criterion::{black_box, criterion_group, criterion_main, Criterion};
use suture_core::repository::Repository;
use tempfile::TempDir;

fn bench_dag_commit_1000(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_perf_commit");

    group.bench_function("commit_1000_files", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let repo = Repository::init(dir.path(), "bench").unwrap();
                for i in 0..1000 {
                    let path = format!("file_{}.txt", i);
                    std::fs::write(dir.path().join(&path), format!("content {}", i)).unwrap();
                }
                (dir, repo)
            },
            |(_dir, mut repo)| {
                for i in 0..1000 {
                    let path = format!("file_{}.txt", i);
                    repo.add(&path).unwrap();
                }
                black_box(repo.commit("commit 1000 files").unwrap());
            },
        );
    });

    group.finish();
}

fn bench_dag_log_1000(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_perf_log");

    group.bench_function("log_1000_commits", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let mut repo = Repository::init(dir.path(), "bench").unwrap();
                for i in 0..1000 {
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

    group.finish();
}

fn bench_dag_log_10000(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_perf_log");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("log_10000_commits", |b| {
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
                let log = repo.log(None).unwrap();
                black_box(&log);
            },
        );
    });

    group.finish();
}

fn bench_dag_merge_100_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_perf_merge");

    group.bench_function("merge_100_files", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let mut repo = Repository::init(dir.path(), "bench").unwrap();

                for i in 0..100 {
                    let path = format!("shared_{}.txt", i);
                    std::fs::write(dir.path().join(&path), format!("base content {}", i)).unwrap();
                    repo.add(&path).unwrap();
                }
                repo.commit("base").unwrap();

                repo.create_branch("feature", None).unwrap();
                repo.checkout("feature").unwrap();
                for i in 0..100 {
                    let path = format!("shared_{}.txt", i);
                    std::fs::write(dir.path().join(&path), format!("feature content {}", i))
                        .unwrap();
                    repo.add(&path).unwrap();
                }
                repo.commit("modify 100 files on feature").unwrap();

                repo.checkout("main").unwrap();
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
    benches,
    bench_dag_commit_1000,
    bench_dag_log_1000,
    bench_dag_log_10000,
    bench_dag_merge_100_files,
);
criterion_main!(benches);
