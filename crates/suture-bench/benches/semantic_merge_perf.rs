use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use suture_driver::SutureDriver;
use suture_driver_json::JsonDriver;

fn generate_json_obj(n: usize) -> String {
    let mut map = serde_json::Map::new();
    for i in 0..n {
        map.insert(
            format!("key_{}", i),
            serde_json::Value::String(format!("value_{}", i)),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap()
}

fn bench_semantic_merge_json_small(c: &mut Criterion) {
    let driver = JsonDriver::new();
    let base = generate_json_obj(10);

    let mut ours_map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&base).unwrap();
    ours_map.insert(
        "key_5".to_string(),
        serde_json::Value::String("modified_by_ours".into()),
    );
    let ours = serde_json::to_string(&serde_json::Value::Object(ours_map)).unwrap();

    let mut theirs_map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&base).unwrap();
    theirs_map.insert(
        "key_3".to_string(),
        serde_json::Value::String("modified_by_theirs".into()),
    );
    theirs_map.insert(
        "new_key".to_string(),
        serde_json::Value::String("new_value".into()),
    );
    let theirs = serde_json::to_string(&serde_json::Value::Object(theirs_map)).unwrap();

    let mut group = c.benchmark_group("semantic_merge_perf");
    group.bench_function("json_small_10_fields", |b| {
        b.iter(|| {
            let _ = black_box(driver.merge(&base, &ours, &theirs).unwrap());
        });
    });
    group.finish();
}

fn bench_semantic_merge_json_large(c: &mut Criterion) {
    let driver = JsonDriver::new();
    let base = generate_json_obj(100);

    let mut ours_map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&base).unwrap();
    ours_map.insert(
        "key_50".to_string(),
        serde_json::Value::String("modified_by_ours".into()),
    );
    let ours = serde_json::to_string(&serde_json::Value::Object(ours_map)).unwrap();

    let mut theirs_map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&base).unwrap();
    theirs_map.insert(
        "key_33".to_string(),
        serde_json::Value::String("modified_by_theirs".into()),
    );
    theirs_map.insert(
        "new_key".to_string(),
        serde_json::Value::String("new_value".into()),
    );
    let theirs = serde_json::to_string(&serde_json::Value::Object(theirs_map)).unwrap();

    let mut group = c.benchmark_group("semantic_merge_perf");
    group.bench_function("json_large_100_fields", |b| {
        b.iter(|| {
            let _ = black_box(driver.merge(&base, &ours, &theirs).unwrap());
        });
    });
    group.finish();
}

fn bench_semantic_merge_json_conflict(c: &mut Criterion) {
    let driver = JsonDriver::new();

    for n in [10usize, 100] {
        let base = generate_json_obj(n);

        let mut ours_map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&base).unwrap();
        ours_map.insert(
            "key_0".to_string(),
            serde_json::Value::String("ours_value".into()),
        );
        let ours = serde_json::to_string(&serde_json::Value::Object(ours_map)).unwrap();

        let mut theirs_map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&base).unwrap();
        theirs_map.insert(
            "key_0".to_string(),
            serde_json::Value::String("theirs_value".into()),
        );
        let theirs = serde_json::to_string(&serde_json::Value::Object(theirs_map)).unwrap();

        let mut group = c.benchmark_group("semantic_merge_perf");
        group.bench_with_input(BenchmarkId::new("json_conflict", n), &n, |b, _| {
            b.iter(|| {
                let result = black_box(driver.merge(&base, &ours, &theirs).unwrap());
                assert!(result.is_none());
            });
        });
        group.finish();
    }
}

criterion_group!(
    benches,
    bench_semantic_merge_json_small,
    bench_semantic_merge_json_large,
    bench_semantic_merge_json_conflict,
);
criterion_main!(benches);
