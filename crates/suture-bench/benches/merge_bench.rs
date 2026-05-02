use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use suture_driver::SutureDriver;
use suture_driver_csv::CsvDriver;
use suture_driver_json::JsonDriver;
use suture_driver_toml::TomlDriver;
use suture_driver_yaml::YamlDriver;

use std::fmt::Write;
fn generate_json(n: usize) -> String {
    let mut map = serde_json::Map::new();
    for i in 0..n {
        map.insert(
            format!("key_{}", i),
            serde_json::Value::String(format!("value_{}", i)),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap()
}

fn generate_yaml(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        let _ = write!(s, "key_{}: value_{}\n", i, i);
    }
    s
}

fn generate_toml(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        let _ = write!(s, "key_{} = \"value_{}\"\n", i, i);
    }
    s
}

fn generate_csv(n: usize) -> String {
    let mut s = String::from("id,name,value\n");
    for i in 0..n {
        let _ = write!(s, "{},item_{},{}\n", i, i, i * 10);
    }
    s
}

fn bench_json_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_bench_json");
    let driver = JsonDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_json(size);
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

        group.bench_with_input(BenchmarkId::new("3way_merge", size), &size, |b, _| {
            b.iter(|| black_box(driver.merge(&base, &ours, &theirs).unwrap()));
        });
    }

    group.finish();
}

fn bench_yaml_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_bench_yaml");
    let driver = YamlDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_yaml(size);
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

        group.bench_with_input(BenchmarkId::new("3way_merge", size), &size, |b, _| {
            b.iter(|| black_box(driver.merge(&base, &ours, &theirs).unwrap()));
        });
    }

    group.finish();
}

fn bench_toml_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_bench_toml");
    let driver = TomlDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_toml(size);
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

        group.bench_with_input(BenchmarkId::new("3way_merge", size), &size, |b, _| {
            b.iter(|| black_box(driver.merge(&base, &ours, &theirs).unwrap()));
        });
    }

    group.finish();
}

fn bench_csv_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_bench_csv");
    let driver = CsvDriver::new();

    for size in [10usize, 100, 1000] {
        let base = generate_csv(size);
        let ours = base.replacen("item_0", "MODIFIED_ITEM", 1);
        let theirs = format!("id,name,value\n999,extra_row,9999\n{}", base);

        group.bench_with_input(BenchmarkId::new("3way_merge", size), &size, |b, _| {
            b.iter(|| black_box(driver.merge(&base, &ours, &theirs).unwrap()));
        });
    }

    group.finish();
}

criterion_group!(
    merge_bench,
    bench_json_merge,
    bench_yaml_merge,
    bench_toml_merge,
    bench_csv_merge,
);
criterion_main!(merge_bench);
