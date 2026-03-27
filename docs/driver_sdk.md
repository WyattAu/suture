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

## Reference Driver: OTIO

The `suture-driver-otio` crate demonstrates the driver pattern for
OpenTimelineIO (.otio) files.

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

To add support for a new file format:

1. Create a new crate: `crates/suture-driver-<format>/`
2. Implement parsing for the format into structured types
3. Assign unique element IDs (e.g., `sheet/rows/3`, `doc/paragraphs/5`)
4. Compute touch sets that cascade from parent to child elements
5. Generate `ChangeDescription` lists from diffs between two versions
6. Serialize patches with the format-specific payload

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
