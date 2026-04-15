# Driver SDK

## Concept

A **SutureDriver** translates between file formats (OTIO, XLSX, DOCX) and
Suture's internal patch model. Each driver is responsible for:

1. **Parsing** a file format into a structured intermediate representation
2. **Computing touch sets** — determining which elements are affected by edits
3. **Serializing patches** — encoding the operation payload for storage
4. **Visual diff** — producing human-readable diffs between two file versions

Drivers do not manage repository state; they only translate between format-
specific representations and Suture's patch algebra.

## Reference Drivers

### OTIO

The `suture-driver-otio` crate demonstrates the driver pattern for
OpenTimelineIO (.otio) files.

### Properties (Example)

The `suture-driver-example` crate provides a minimal, self-contained example
of implementing the `SutureDriver` trait for Java `.properties` files. It
covers parsing, semantic diffing, and three-way merge — making it the
recommended starting point for anyone writing a custom driver.

### Parsing

```rust
use suture_driver_otio::OtioDriver;

let mut driver = OtioDriver::new();
driver.parse_otio(&otio_json)?;

for elem in driver.elements() {
    println!("[{}] {} ({})", elem.element_type(), elem.name(), elem.id());
}
```

### Computing Touch Sets

When a user edits a timeline, the driver computes which timeline elements are
affected:

```rust
use suture_driver_otio::{OtioDriver, ChangeDescription};

let mut driver = OtioDriver::new();
driver.parse_otio(&otio_json)?;

let changes = vec![ChangeDescription {
    element_id: "timeline/track".to_string(),
    field_path: "name".to_string(),
    old_value: Some("Video".to_string()),
    new_value: Some("Audio".to_string()),
}];

let touch_set = driver.compute_touch_set(&changes);
// Returns affected element IDs, cascading to children
```

### Visual Diff

```rust
let diff = driver.serialize_diff(&old_json, &new_json)?;
println!("{}", diff);
```

## Implementing a Custom Driver

See `crates/suture-driver-example/` for a minimal working driver. The general
steps are:

1. Create a new crate: `crates/suture-driver-<format>/`
2. Depend on `suture-driver` and implement the `SutureDriver` trait
3. Implement `diff()` to produce `SemanticChange` values between two file versions
4. Implement `format_diff()` for human-readable output
5. Implement `merge()` for three-way semantic merge (return `None` on conflict)
6. Return supported extensions from `supported_extensions()`

### Element ID Convention

Use slash-delimited paths that reflect the document hierarchy:

| Format     | Example Element IDs                          |
|------------|----------------------------------------------|
| OTIO       | `timeline/track/clip`, `timeline/stack`      |
| XLSX       | `workbook/sheet/rows/3/cols/A`               |
| DOCX       | `doc/body/paragraphs/5/runs/2`               |

### Commutativity

Two patches commute when their touch sets are disjoint. For OTIO, editing
clip A and clip B commute because they touch different elements. Editing
the same clip twice does not commute.

This property is format-agnostic — the driver only needs to compute correct
touch sets. Suture's patch algebra handles the rest.
