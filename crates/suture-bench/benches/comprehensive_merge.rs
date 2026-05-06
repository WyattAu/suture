use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use suture_driver::SutureDriver;
use suture_driver_csv::CsvDriver;
use suture_driver_json::JsonDriver;
use suture_driver_toml::TomlDriver;
use suture_driver_xml::XmlDriver;
use suture_driver_yaml::YamlDriver;

// =============================================================================
// JSON generators
// =============================================================================

use std::fmt::Write;
fn generate_json(n: usize) -> String {
    let mut map = serde_json::Map::new();
    for i in 0..n {
        map.insert(
            format!("key_{}", i),
            serde_json::json!(format!("value_{}", i)),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap()
}

fn json_modify_keys(content: &str, keys: &[usize], prefix: &str) -> String {
    let mut map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(content).unwrap();
    for &i in keys {
        map.insert(
            format!("key_{}", i),
            serde_json::json!(format!("{}_value_{}", prefix, i)),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap()
}

// =============================================================================
// YAML generators
// =============================================================================

fn generate_yaml(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        let _ = write!(s, "key_{}: value_{}\n", i, i);
    }
    s
}

fn yaml_modify_keys(content: &str, keys: &[usize], prefix: &str) -> String {
    let mut result = content.to_string();
    for &i in keys {
        result = result.replacen(
            &format!("key_{}: value_{}", i, i),
            &format!("key_{}: {}_value_{}", i, prefix, i),
            1,
        );
    }
    result
}

// =============================================================================
// TOML generators
// =============================================================================

fn generate_toml(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        let _ = write!(s, "key_{} = \"value_{}\"\n", i, i);
    }
    s
}

fn toml_modify_keys(content: &str, keys: &[usize], prefix: &str) -> String {
    let mut result = content.to_string();
    for &i in keys {
        result = result.replacen(
            &format!("key_{} = \"value_{}\"", i, i),
            &format!("key_{} = \"{}_value_{}\"", i, prefix, i),
            1,
        );
    }
    result
}

// =============================================================================
// CSV generators
// =============================================================================

fn generate_csv(rows: usize, cols: usize) -> String {
    let mut header = String::from("id");
    for c in 1..cols {
        let _ = write!(header, ",col_{}", c);
    }
    header.push('\n');

    let mut body = String::new();
    for r in 0..rows {
        let _ = write!(body, "{}", r);
        for c in 1..cols {
            let _ = write!(body, ",val_{}_{}", r, c);
        }
        body.push('\n');
    }
    format!("{}{}", header, body)
}

// =============================================================================
// XML generators
// =============================================================================

fn generate_xml(n: usize) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n");
    for i in 0..n {
        let _ = write!(s, "  <item id=\"{}\">value_{}</item>\n", i, i);
    }
    s.push_str("</root>\n");
    s
}

fn xml_modify_elements(content: &str, ids: &[usize], prefix: &str) -> String {
    let mut result = content.to_string();
    for &id in ids {
        result = result.replacen(
            &format!("<item id=\"{}\">value_{}</item>", id, id),
            &format!("<item id=\"{}\">{}_value_{}</item>", id, prefix, id),
            1,
        );
    }
    result
}

// =============================================================================
// JSON merge benchmarks
// =============================================================================

fn bench_json_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_merge");
    group.sample_size(50);

    for size in [10usize, 100, 1000, 10_000] {
        let base = generate_json(size);

        let ours_same = base.clone();
        let theirs_same = base.clone();

        let ours_one_side = json_modify_keys(&base, &[0], "ours");
        let theirs_unchanged = base.clone();

        let ours_diff_keys = json_modify_keys(&base, &(0..size / 2).collect::<Vec<_>>(), "ours");
        let theirs_diff_keys =
            json_modify_keys(&base, &(size / 2..size).collect::<Vec<_>>(), "theirs");

        let ours_conflict = json_modify_keys(&base, &[size / 2], "ours");
        let theirs_conflict = json_modify_keys(&base, &[size / 2], "theirs");

        let driver = JsonDriver::new();

        group.bench_with_input(BenchmarkId::new("same_change", size), &size, |b, _| {
            b.iter(|| {
                black_box(driver.merge(&base, &ours_same, &theirs_same).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("one_sided", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_one_side, &theirs_unchanged)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("different_keys", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_diff_keys, &theirs_diff_keys)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("conflict", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_conflict, &theirs_conflict)
                        .unwrap(),
                );
            });
        });
    }

    group.finish();
}

// =============================================================================
// YAML merge benchmarks
// =============================================================================

fn bench_yaml_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("yaml_merge");
    group.sample_size(50);

    for size in [10usize, 50, 200] {
        let base = generate_yaml(size);

        let ours_same = base.clone();
        let theirs_same = base.clone();

        let ours_one_side = yaml_modify_keys(&base, &[0], "ours");
        let theirs_unchanged = base.clone();

        let ours_diff_keys = yaml_modify_keys(&base, &(0..size / 2).collect::<Vec<_>>(), "ours");
        let theirs_diff_keys =
            yaml_modify_keys(&base, &(size / 2..size).collect::<Vec<_>>(), "theirs");

        let ours_conflict = yaml_modify_keys(&base, &[size / 2], "ours");
        let theirs_conflict = yaml_modify_keys(&base, &[size / 2], "theirs");

        let driver = YamlDriver::new();

        group.bench_with_input(BenchmarkId::new("same_change", size), &size, |b, _| {
            b.iter(|| {
                black_box(driver.merge(&base, &ours_same, &theirs_same).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("one_sided", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_one_side, &theirs_unchanged)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("different_keys", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_diff_keys, &theirs_diff_keys)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("conflict", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_conflict, &theirs_conflict)
                        .unwrap(),
                );
            });
        });
    }

    group.finish();
}

// =============================================================================
// TOML merge benchmarks
// =============================================================================

fn bench_toml_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("toml_merge");
    group.sample_size(50);

    for size in [10usize, 50] {
        let base = generate_toml(size);

        let ours_same = base.clone();
        let theirs_same = base.clone();

        let ours_one_side = toml_modify_keys(&base, &[0], "ours");
        let theirs_unchanged = base.clone();

        let ours_diff_keys = toml_modify_keys(&base, &(0..size / 2).collect::<Vec<_>>(), "ours");
        let theirs_diff_keys =
            toml_modify_keys(&base, &(size / 2..size).collect::<Vec<_>>(), "theirs");

        let ours_conflict = toml_modify_keys(&base, &[size / 2], "ours");
        let theirs_conflict = toml_modify_keys(&base, &[size / 2], "theirs");

        let driver = TomlDriver::new();

        group.bench_with_input(BenchmarkId::new("same_change", size), &size, |b, _| {
            b.iter(|| {
                black_box(driver.merge(&base, &ours_same, &theirs_same).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("one_sided", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_one_side, &theirs_unchanged)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("different_keys", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_diff_keys, &theirs_diff_keys)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("conflict", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_conflict, &theirs_conflict)
                        .unwrap(),
                );
            });
        });
    }

    group.finish();
}

// =============================================================================
// CSV merge benchmarks
// =============================================================================

fn bench_csv_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_merge");
    group.sample_size(50);

    let sizes: Vec<(usize, usize)> = vec![(10, 5), (100, 10), (1000, 20)];

    for (rows, cols) in sizes {
        let base = generate_csv(rows, cols);
        let label = format!("{}r_{}c", rows, cols);

        let ours_same = base.clone();
        let theirs_same = base.clone();

        let ours_one_side = base.replacen("val_0_1", "MODIFIED", 1);
        let theirs_unchanged = base.clone();

        let theirs_diff_row = base.replacen("val_1_1", "THEIRS_MODIFIED", 1);

        let ours_conflict = base.replacen("val_0_1", "OURS", 1);
        let theirs_conflict = base.replacen("val_0_1", "THEIRS", 1);

        let driver = CsvDriver::new();

        group.bench_with_input(BenchmarkId::new("same_change", &label), &label, |b, _| {
            b.iter(|| {
                black_box(driver.merge(&base, &ours_same, &theirs_same).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("one_sided", &label), &label, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_one_side, &theirs_unchanged)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(
            BenchmarkId::new("different_keys", &label),
            &label,
            |b, _| {
                b.iter(|| {
                    black_box(
                        driver
                            .merge(
                                &base,
                                &theirs_diff_row,
                                &base.replacen("val_2_1", "THEIRS", 1),
                            )
                            .unwrap(),
                    );
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("conflict", &label), &label, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_conflict, &theirs_conflict)
                        .unwrap(),
                );
            });
        });
    }

    group.finish();
}

// =============================================================================
// XML merge benchmarks
// =============================================================================

fn bench_xml_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("xml_merge");
    group.sample_size(50);

    for size in [10usize, 100] {
        let base = generate_xml(size);

        let ours_same = base.clone();
        let theirs_same = base.clone();

        let ours_one_side = xml_modify_elements(&base, &[0], "ours");
        let theirs_unchanged = base.clone();

        let ours_diff_keys = xml_modify_elements(&base, &(0..size / 2).collect::<Vec<_>>(), "ours");
        let theirs_diff_keys =
            xml_modify_elements(&base, &(size / 2..size).collect::<Vec<_>>(), "theirs");

        let ours_conflict = xml_modify_elements(&base, &[size / 2], "ours");
        let theirs_conflict = xml_modify_elements(&base, &[size / 2], "theirs");

        let driver = XmlDriver::new();

        group.bench_with_input(BenchmarkId::new("same_change", size), &size, |b, _| {
            b.iter(|| {
                black_box(driver.merge(&base, &ours_same, &theirs_same).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("one_sided", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_one_side, &theirs_unchanged)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("different_keys", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_diff_keys, &theirs_diff_keys)
                        .unwrap(),
                );
            });
        });

        group.bench_with_input(BenchmarkId::new("conflict", size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    driver
                        .merge(&base, &ours_conflict, &theirs_conflict)
                        .unwrap(),
                );
            });
        });
    }

    group.finish();
}

criterion_group!(
    comprehensive_merge,
    bench_json_merge,
    bench_yaml_merge,
    bench_toml_merge,
    bench_csv_merge,
    bench_xml_merge,
);
criterion_main!(comprehensive_merge);
