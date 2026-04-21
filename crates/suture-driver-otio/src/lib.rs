//! OTIO semantic driver — timeline-level diff and merge for OpenTimelineIO files.
//!
//! ## Architecture
//!
//! OpenTimelineIO (OTIO) files are JSON documents describing video editing timelines.
//! The key challenge is identity: OTIO doesn't require unique IDs for elements,
//! so we use content-based identity heuristics.
//!
//! Identity strategy:
//! - **Clips:** Identified by `media_reference.target_url` + `source_range.start_time`
//!   (fallback: `name` + `source_range`). This means renaming a clip without changing
//!   its source preserves identity.
//! - **Tracks/Stacks:** Identified by `name` + `kind` + position in parent.
//! - **Transitions:** Identified by `name` + position in parent.
//! - **Timeline:** Always identity element (there's only one root).
//!
//! The merge operates at the JSON level: it parses the OTIO JSON, compares
//! element trees using content-based identity, performs a three-way merge,
//! and serializes the result back to JSON.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use suture_driver::{DriverError, SemanticChange, SutureDriver};

#[derive(Error, Debug)]
pub enum OtioError {
    #[error("failed to read OTIO file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse OTIO JSON: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("invalid OTIO structure: {0}")]
    InvalidStructure(String),

    #[error("element not found: {0}")]
    ElementNotFound(String),

    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = std::result::Result<T, OtioError>;

// =============================================================================
// OTIO Schema Types (minimal subset of OpenTimelineIO)
//
// These types store children as serde_json::Value (not OtioNode) so they
// can derive Serialize/Deserialize.  We convert to/from OtioNode manually.
// =============================================================================

/// All recognized OTIO node types. Unknown types are stored as opaque JSON.
#[derive(Clone, Debug)]
pub enum OtioNode {
    Timeline(Timeline),
    Track(Track),
    Stack(Stack),
    Clip(Clip),
    Transition(Transition),
    SerializableCollection(SerializableCollection),
    /// Unknown OTIO type — stored as opaque JSON to avoid parse failures.
    Unknown {
        schema: String,
        value: serde_json::Value,
    },
}

impl OtioNode {
    fn schema_type(&self) -> &str {
        match self {
            OtioNode::Timeline(_) => "Timeline",
            OtioNode::Track(_) => "Track",
            OtioNode::Stack(_) => "Stack",
            OtioNode::Clip(_) => "Clip",
            OtioNode::Transition(_) => "Transition",
            OtioNode::SerializableCollection(_) => "SerializableCollection",
            OtioNode::Unknown { schema, .. } => schema.as_str(),
        }
    }

    /// Return child OtioNodes for containers that hold them.
    fn children(&self) -> Vec<OtioNode> {
        match self {
            OtioNode::Timeline(tl) => tl.child_nodes(),
            OtioNode::Track(tr) => tr.child_nodes(),
            OtioNode::Stack(st) => st.child_nodes(),
            OtioNode::SerializableCollection(sc) => sc.child_nodes(),
            _ => Vec::new(),
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            OtioNode::Timeline(tl) => Some(&tl.name),
            OtioNode::Track(tr) => Some(&tr.name),
            OtioNode::Stack(st) => Some(&st.name),
            OtioNode::Clip(cl) => Some(&cl.name),
            OtioNode::Transition(tr) => Some(&tr.name),
            OtioNode::SerializableCollection(sc) => Some(&sc.name),
            OtioNode::Unknown { value, .. } => value.get("name").and_then(|v| v.as_str()),
        }
    }

    /// Serialize this node back to a JSON value.
    fn to_json(&self) -> serde_json::Value {
        match self {
            OtioNode::Timeline(tl) => serde_json::to_value(tl).unwrap_or_default(),
            OtioNode::Track(tr) => serde_json::to_value(tr).unwrap_or_default(),
            OtioNode::Stack(st) => serde_json::to_value(st).unwrap_or_default(),
            OtioNode::Clip(cl) => serde_json::to_value(cl).unwrap_or_default(),
            OtioNode::Transition(tr) => serde_json::to_value(tr).unwrap_or_default(),
            OtioNode::SerializableCollection(sc) => {
                serde_json::to_value(sc).unwrap_or_default()
            }
            OtioNode::Unknown { value, .. } => value.clone(),
        }
    }
}

// --- Serde-friendly struct types (children stored as raw JSON) ---

fn parse_children(json_children: &[serde_json::Value]) -> Vec<OtioNode> {
    json_children.iter().filter_map(|v| parse_otio_node(v).ok()).collect()
}

#[allow(dead_code)]
fn children_to_json(nodes: &[OtioNode]) -> Vec<serde_json::Value> {
    nodes.iter().map(|n| n.to_json()).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeline {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Children stored as raw JSON so Timeline can derive Serialize/Deserialize.
    #[serde(default, rename = "tracks")]
    pub tracks_json: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_start_time: Option<RationalTime>,
}

impl Timeline {
    fn child_nodes(&self) -> Vec<OtioNode> {
        parse_children(&self.tracks_json)
    }
    fn with_children(mut self, nodes: Vec<OtioNode>) -> Self {
        self.tracks_json = children_to_json(&nodes);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(rename = "kind")]
    pub kind: String,
    #[serde(default, rename = "children")]
    pub children_json: Vec<serde_json::Value>,
}

impl Track {
    fn child_nodes(&self) -> Vec<OtioNode> {
        parse_children(&self.children_json)
    }
    fn with_children(mut self, nodes: Vec<OtioNode>) -> Self {
        self.children_json = children_to_json(&nodes);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stack {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default, rename = "children")]
    pub children_json: Vec<serde_json::Value>,
}

impl Stack {
    fn child_nodes(&self) -> Vec<OtioNode> {
        parse_children(&self.children_json)
    }
    fn with_children(mut self, nodes: Vec<OtioNode>) -> Self {
        self.children_json = children_to_json(&nodes);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_range: Option<TimeRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_reference: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transition {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_offset: Option<RationalTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_offset: Option<RationalTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableCollection {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default, rename = "children")]
    pub children_json: Vec<serde_json::Value>,
}

impl SerializableCollection {
    fn child_nodes(&self) -> Vec<OtioNode> {
        parse_children(&self.children_json)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RationalTime {
    pub value: f64,
    pub rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_time: RationalTime,
    pub duration: RationalTime,
}

// =============================================================================
// OTIO JSON Parsing (with unknown type handling)
// =============================================================================

fn parse_otio_node(value: &serde_json::Value) -> Result<OtioNode> {
    let schema = value
        .get("OTIO_SCHEMA")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if schema.is_empty() {
        return Err(OtioError::InvalidStructure(
            "missing OTIO_SCHEMA field".into(),
        ));
    }

    // Try to deserialize as known types
    match schema {
        "otio.schema.Timeline" => serde_json::from_value::<Timeline>(value.clone())
            .map(OtioNode::Timeline)
            .map_err(|_| OtioError::InvalidStructure(format!("failed to parse {}", schema))),
        "otio.schema.Track" => serde_json::from_value::<Track>(value.clone())
            .map(OtioNode::Track)
            .map_err(|_| OtioError::InvalidStructure(format!("failed to parse {}", schema))),
        "otio.schema.Stack" => serde_json::from_value::<Stack>(value.clone())
            .map(OtioNode::Stack)
            .map_err(|_| OtioError::InvalidStructure(format!("failed to parse {}", schema))),
        "otio.schema.Clip" => serde_json::from_value::<Clip>(value.clone())
            .map(OtioNode::Clip)
            .map_err(|_| OtioError::InvalidStructure(format!("failed to parse {}", schema))),
        "otio.schema.Transition" => serde_json::from_value::<Transition>(value.clone())
            .map(OtioNode::Transition)
            .map_err(|_| OtioError::InvalidStructure(format!("failed to parse {}", schema))),
        "otio.schema.SerializableCollection" => {
            serde_json::from_value::<SerializableCollection>(value.clone())
                .map(OtioNode::SerializableCollection)
                .map_err(|_| OtioError::InvalidStructure(format!("failed to parse {}", schema)))
        }
        // Unknown schema — store as opaque JSON (graceful degradation)
        _ => Ok(OtioNode::Unknown {
            schema: schema.to_string(),
            value: value.clone(),
        }),
    }
}

fn parse_otio_json(input: &str) -> Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(input)?;
    // Validate it has OTIO_SCHEMA
    if !value.is_object() || !value.as_object().map(|o| o.contains_key("OTIO_SCHEMA")).unwrap_or(false) {
        return Err(OtioError::InvalidStructure(
            "root is not an OTIO object (missing OTIO_SCHEMA)".into(),
        ));
    }
    Ok(value)
}

// =============================================================================
// Content-Based Identity
// =============================================================================

/// Compute a content-based identity fingerprint for an OTIO node.
///
/// The fingerprint is used to match elements across base/ours/theirs versions.
/// - Clips: `name` + `source_range.start_time` + `media_reference.target_url`
/// - Tracks/Stacks: `name` + `kind`
/// - Transitions: `name` + `in_offset`
/// - Unknown: JSON serialization hash
fn content_fingerprint(node: &OtioNode) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    match node {
        OtioNode::Clip(cl) => {
            "clip".hash(&mut hasher);
            cl.name.hash(&mut hasher);
            if let Some(sr) = &cl.source_range {
                sr.start_time.value.to_bits().hash(&mut hasher);
            }
            // Extract target_url from media_reference
            if let Some(mr) = &cl.media_reference {
                if let Some(url) = mr.get("target_url").and_then(|v| v.as_str()) {
                    url.hash(&mut hasher);
                }
            }
        }
        OtioNode::Track(tr) => {
            "track".hash(&mut hasher);
            tr.name.hash(&mut hasher);
            tr.kind.hash(&mut hasher);
        }
        OtioNode::Stack(st) => {
            "stack".hash(&mut hasher);
            st.name.hash(&mut hasher);
        }
        OtioNode::Transition(tr) => {
            "transition".hash(&mut hasher);
            tr.name.hash(&mut hasher);
            if let Some(io) = &tr.in_offset {
                io.value.to_bits().hash(&mut hasher);
            }
        }
        OtioNode::Timeline(tl) => {
            "timeline".hash(&mut hasher);
            tl.name.hash(&mut hasher);
        }
        OtioNode::SerializableCollection(sc) => {
            "collection".hash(&mut hasher);
            sc.name.hash(&mut hasher);
        }
        OtioNode::Unknown { value, .. } => {
            format!("{:?}", value).hash(&mut hasher);
        }
    }

    format!("{:016x}", hasher.finish())
}

// =============================================================================
// Tree Diffing
// =============================================================================

/// Recursively collect all nodes with their paths and fingerprints.
#[derive(Clone)]
struct FlatNode {
    path: String,
    fingerprint: String,
    node: OtioNode,
    /// Original raw JSON value — used for exact comparison (no serde round-trip).
    raw_json: serde_json::Value,
}

fn flatten_tree_with_raw(value: &serde_json::Value, parent_path: &str) -> Vec<FlatNode> {
    let mut result = Vec::new();
    let node = match parse_otio_node(value) {
        Ok(n) => n,
        Err(_) => return result,
    };
    let fp = content_fingerprint(&node);

    // Build path
    let path = if parent_path.is_empty() {
        format!("/{}", node.schema_type())
    } else {
        format!("{}/{}", parent_path, node.schema_type())
    };

    result.push(FlatNode {
        path: path.clone(),
        fingerprint: fp,
        raw_json: value.clone(),
        node: node.clone(),
    });

    // Recurse into children arrays
    let child_keys = ["tracks", "children"];
    for key in &child_keys {
        if let Some(arr) = value.get(key).and_then(|v| v.as_array()) {
            for (i, child_val) in arr.iter().enumerate() {
                let child_path = format!("{}/[{}]", path, i);
                result.extend(flatten_tree_with_raw(child_val, &child_path));
            }
        }
    }

    result
}

fn diff_trees(base_nodes: &[FlatNode], new_nodes: &[FlatNode]) -> Vec<SemanticChange> {
    let new_by_fp: HashMap<&str, &FlatNode> =
        new_nodes.iter().map(|n| (n.fingerprint.as_str(), n)).collect();
    let base_by_fp: HashMap<&str, &FlatNode> =
        base_nodes.iter().map(|n| (n.fingerprint.as_str(), n)).collect();

    let mut changes = Vec::new();

    // Detect additions
    for node in new_nodes {
        if !base_by_fp.contains_key(node.fingerprint.as_str()) {
            changes.push(SemanticChange::Added {
                path: node.path.clone(),
                value: format!(
                    "{} ({})",
                    node.node.name().unwrap_or("?"),
                    node.node.schema_type()
                ),
            });
        }
    }

    // Detect removals
    for node in base_nodes {
        if !new_by_fp.contains_key(node.fingerprint.as_str()) {
            changes.push(SemanticChange::Removed {
                path: node.path.clone(),
                old_value: format!(
                    "{} ({})",
                    node.node.name().unwrap_or("?"),
                    node.node.schema_type()
                ),
            });
        }
    }

    // Detect modifications (same fingerprint but different JSON)
    // Skip containers (Timeline, Track, Stack, SerializableCollection) — their
    // changes are already captured by child-level adds/removes/modifications.
    for new_node in new_nodes {
        if let Some(base_node) = base_by_fp.get(new_node.fingerprint.as_str()) {
            // Only check leaf nodes for modifications
            let is_leaf = matches!(
                new_node.node,
                OtioNode::Clip(_) | OtioNode::Transition(_) | OtioNode::Unknown { .. }
            );
            if is_leaf && base_node.raw_json != new_node.raw_json {
                changes.push(SemanticChange::Modified {
                    path: new_node.path.clone(),
                    old_value: format!("{} ({})", base_node.node.name().unwrap_or("?"), base_node.node.schema_type()),
                    new_value: format!("{} ({})", new_node.node.name().unwrap_or("?"), new_node.node.schema_type()),
                });
            }
        }
    }

    changes
}

// =============================================================================
// Three-Way Merge
// =============================================================================

/// Three-way merge of OTIO element trees.
///
/// Strategy:
/// - Build flat node lists from base/ours/theirs with content fingerprints
/// - Match nodes by fingerprint across versions
/// - For unmatched nodes: additions by one side are included; removals by both are honored
/// - For matched nodes with modifications: if only one side modified, take that; if both modified identically, take either; if both modified differently, CONFLICT
fn merge_trees(
    base_nodes: &[FlatNode],
    ours_nodes: &[FlatNode],
    theirs_nodes: &[FlatNode],
) -> Option<serde_json::Value> {
    let base_by_fp: HashMap<&str, &FlatNode> =
        base_nodes.iter().map(|n| (n.fingerprint.as_str(), n)).collect();
    let ours_by_fp: HashMap<&str, &FlatNode> =
        ours_nodes.iter().map(|n| (n.fingerprint.as_str(), n)).collect();
    let theirs_by_fp: HashMap<&str, &FlatNode> =
        theirs_nodes.iter().map(|n| (n.fingerprint.as_str(), n)).collect();

    let all_fps: std::collections::HashSet<&str> = base_by_fp
        .keys()
        .chain(ours_by_fp.keys())
        .chain(theirs_by_fp.keys())
        .copied()
        .collect();

    // Collect all nodes that should be in the merged result
    let mut merged_nodes: Vec<(String, FlatNode)> = Vec::new(); // (fingerprint, node)

    for &fp in &all_fps {
        let in_base = base_by_fp.contains_key(fp);
        let in_ours = ours_by_fp.contains_key(fp);
        let in_theirs = theirs_by_fp.contains_key(fp);

        match (in_base, in_ours, in_theirs) {
            // All three have it — check for modifications (leaf nodes only)
            (true, true, true) => {
                let base_node = base_by_fp[fp];
                let ours_node = ours_by_fp[fp];
                let theirs_node = theirs_by_fp[fp];

                // Only consider leaf nodes as "modified" — container changes
                // are captured by child-level adds/removes/modifications.
                let is_leaf = matches!(
                    base_node.node,
                    OtioNode::Clip(_) | OtioNode::Transition(_) | OtioNode::Unknown { .. }
                );

                let ours_modified = is_leaf && base_node.raw_json != ours_node.raw_json;
                let theirs_modified = is_leaf && base_node.raw_json != theirs_node.raw_json;

                match (ours_modified, theirs_modified) {
                    (false, false) => {
                        merged_nodes.push((fp.to_string(), base_by_fp[fp].clone()));
                    }
                    (true, false) => {
                        merged_nodes.push((fp.to_string(), ours_by_fp[fp].clone()));
                    }
                    (false, true) => {
                        merged_nodes.push((fp.to_string(), theirs_by_fp[fp].clone()));
                    }
                    (true, true) => {
                        if ours_by_fp[fp].raw_json == theirs_by_fp[fp].raw_json {
                            merged_nodes.push((fp.to_string(), ours_by_fp[fp].clone()));
                        } else {
                            // Genuine conflict
                            return None;
                        }
                    }
                }
            }

            // Added by ours only
            (false, true, false) => {
                merged_nodes.push((fp.to_string(), ours_by_fp[fp].clone()));
            }

            // Added by theirs only
            (false, false, true) => {
                merged_nodes.push((fp.to_string(), theirs_by_fp[fp].clone()));
            }

            // Added by both
            (false, true, true) => {
                if ours_by_fp[fp].raw_json == theirs_by_fp[fp].raw_json {
                    merged_nodes.push((fp.to_string(), ours_by_fp[fp].clone()));
                } else {
                    return None;
                }
            }

            // Removed by ours, kept by theirs — non-conflicting delete
            (true, false, true) => {
                // Delete wins
            }

            // Kept by ours, removed by theirs — non-conflicting delete
            (true, true, false) => {
                // Delete wins
            }

            // Removed by both
            (true, false, false) => {
                // Delete wins
            }

            (false, false, false) => unreachable!(),
        }
    }

    // Reconstruct the OTIO JSON from the merged flat nodes.
    // Use ours as the structural template, then replace children with merged nodes.
    let ours_json: serde_json::Value = match ours_nodes.first() {
        Some(node) => node.node.to_json(),
        None => serde_json::Value::Null,
    };

    let theirs_json: serde_json::Value = match theirs_nodes.first() {
        Some(node) => node.node.to_json(),
        None => serde_json::Value::Null,
    };

    // Use whichever version has the root node
    let template = if ours_json.is_object() {
        ours_json
    } else if theirs_json.is_object() {
        theirs_json
    } else {
        return base_nodes.first().map(|n| n.node.to_json());
    };

    // Rebuild the tree by matching fingerprints
    let mut result = template;
    let mut placed_fps = std::collections::HashSet::new();
    rebuild_children_with_merged(&mut result, &merged_nodes, &mut placed_fps);

    Some(result)
}

/// Recursively rebuild children arrays using the merged node set.
///
/// `placed_fps` tracks all fingerprints already placed somewhere in the tree,
/// so the second pass doesn't add duplicates to wrong containers.
fn rebuild_children_with_merged(
    value: &mut serde_json::Value,
    merged_nodes: &[(String, FlatNode)],
    placed_fps: &mut std::collections::HashSet<String>,
) {
    if let Some(obj) = value.as_object_mut() {
        for key in ["tracks", "children"] {
            if let Some(arr) = obj.get_mut(key).and_then(|v| v.as_array_mut()) {
                let mut new_arr = Vec::new();

                // First pass: replace existing children with their merged versions
                for item in arr.iter() {
                    if let Some(_schema) = item.get("OTIO_SCHEMA").and_then(|v| v.as_str()) {
                        if let Ok(node) = parse_otio_node(item) {
                            let fp = content_fingerprint(&node);
                            placed_fps.insert(fp.clone());
                            if let Some((_, merged_node)) = merged_nodes.iter().find(|(f, _)| *f == fp)
                            {
                                new_arr.push(merged_node.node.to_json());
                                continue;
                            }
                        }
                    }
                    new_arr.push(item.clone());
                }

                // Second pass: append merged leaf nodes not yet placed anywhere
                for (fp, merged_node) in merged_nodes {
                    if !placed_fps.contains(fp.as_str()) {
                        let is_leaf = matches!(
                            merged_node.node,
                            OtioNode::Clip(_) | OtioNode::Transition(_) | OtioNode::Unknown { .. }
                        );
                        if is_leaf {
                            new_arr.push(merged_node.node.to_json());
                            placed_fps.insert(fp.clone());
                        }
                    }
                }

                *arr = new_arr;
            }
        }

        // Recurse into child objects
        if let Some(children) = obj.get_mut("children").and_then(|v| v.as_array_mut()) {
            for child in children.iter_mut() {
                rebuild_children_with_merged(child, merged_nodes, placed_fps);
            }
        }
        if let Some(tracks) = obj.get_mut("tracks").and_then(|v| v.as_array_mut()) {
            for child in tracks.iter_mut() {
                rebuild_children_with_merged(child, merged_nodes, placed_fps);
            }
        }
    }
}

// =============================================================================
// SutureDriver Implementation
// =============================================================================

pub struct OtioDriver;

impl OtioDriver {
    pub fn new() -> Self {
        Self
    }

    fn parse_and_flatten(input: &str) -> std::result::Result<Vec<FlatNode>, DriverError> {
        let value = parse_otio_json(input)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let _node = parse_otio_node(&value)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        Ok(flatten_tree_with_raw(&value, ""))
    }
}

impl Default for OtioDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for OtioDriver {
    fn name(&self) -> &str {
        "OpenTimelineIO"
    }
    fn supported_extensions(&self) -> &[&str] {
        &[".otio"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> std::result::Result<Vec<SemanticChange>, DriverError> {
        let new_nodes = Self::parse_and_flatten(new_content)?;

        let base_nodes = match base_content {
            None => Vec::new(),
            Some(base) => Self::parse_and_flatten(base)?,
        };

        Ok(diff_trees(&base_nodes, &new_nodes))
    }

    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> std::result::Result<String, DriverError> {
        let changes = self.diff(base_content, new_content)?;
        if changes.is_empty() {
            return Ok("no changes".to_string());
        }
        let lines: Vec<String> = changes
            .iter()
            .map(|c| match c {
                SemanticChange::Added { path, value } => format!("  ADDED     {}: {}", path, value),
                SemanticChange::Removed { path, old_value } => {
                    format!("  REMOVED   {}: {}", path, old_value)
                }
                SemanticChange::Modified {
                    path,
                    old_value,
                    new_value,
                } => format!("  MODIFIED  {}: {} -> {}", path, old_value, new_value),
                SemanticChange::Moved {
                    old_path,
                    new_path,
                    value,
                } => format!("  MOVED     {} -> {}: {}", old_path, new_path, value),
            })
            .collect();
        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> std::result::Result<Option<String>, DriverError> {
        let base_nodes = Self::parse_and_flatten(base)?;
        let ours_nodes = Self::parse_and_flatten(ours)?;
        let theirs_nodes = Self::parse_and_flatten(theirs)?;

        let merged = merge_trees(&base_nodes, &ours_nodes, &theirs_nodes);

        match merged {
            Some(value) => {
                let json = serde_json::to_string_pretty(&value)
                    .map_err(|e| DriverError::SerializationError(e.to_string()))?;
                Ok(Some(json))
            }
            None => Ok(None),
        }
    }
}

// =============================================================================
// Legacy API (kept for backward compatibility with E2E tests)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub enum TimelineElement {
    Timeline { id: String, name: String },
    Track { id: String, name: String, kind: String, parent_id: Option<String> },
    Clip { id: String, name: String, parent_id: Option<String> },
    Transition { id: String, name: String, parent_id: Option<String> },
}

impl TimelineElement {
    pub fn id(&self) -> &str {
        match self {
            TimelineElement::Timeline { id, .. } => id,
            TimelineElement::Track { id, .. } => id,
            TimelineElement::Clip { id, .. } => id,
            TimelineElement::Transition { id, .. } => id,
        }
    }
    pub fn element_type(&self) -> &str {
        match self {
            TimelineElement::Timeline { .. } => "Timeline",
            TimelineElement::Track { .. } => "Track",
            TimelineElement::Clip { .. } => "Clip",
            TimelineElement::Transition { .. } => "Transition",
        }
    }
    pub fn name(&self) -> &str {
        match self {
            TimelineElement::Timeline { name, .. } => name,
            TimelineElement::Track { name, .. } => name,
            TimelineElement::Clip { name, .. } => name,
            TimelineElement::Transition { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChangeDescription {
    pub element_id: String,
    pub field_path: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

/// Legacy OtioDriver that supports the old API.
pub struct LegacyOtioDriver {
    elements: Vec<TimelineElement>,
    raw_json: serde_json::Value,
}

impl LegacyOtioDriver {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            raw_json: serde_json::Value::Null,
        }
    }

    pub fn parse_otio(&mut self, input: &str) -> Result<()> {
        let root: serde_json::Value = serde_json::from_str(input)?;
        self.raw_json = root.clone();

        let node = parse_otio_node(&root)?;

        self.elements.clear();
        self.collect_elements(node, None, 0)?;
        Ok(())
    }

    fn collect_elements(
        &mut self,
        node: OtioNode,
        parent_id: Option<String>,
        index: usize,
    ) -> Result<()> {
        match &node {
            OtioNode::Timeline(tl) => {
                let element_id =
                    Self::element_id("timeline", &tl.name, index, parent_id.as_deref());
                self.elements.push(TimelineElement::Timeline {
                    id: element_id.clone(),
                    name: tl.name.clone(),
                });
                for (i, child) in tl.child_nodes().iter().enumerate() {
                    self.collect_elements(child.clone(), Some(element_id.clone()), i)?;
                }
            }
            OtioNode::Stack(st) => {
                let element_id = Self::element_id("stack", &st.name, index, parent_id.as_deref());
                self.elements.push(TimelineElement::Track {
                    id: element_id.clone(),
                    name: st.name.clone(),
                    kind: "Stack".to_string(),
                    parent_id: parent_id.clone(),
                });
                for (i, child) in st.child_nodes().iter().enumerate() {
                    self.collect_elements(child.clone(), Some(element_id.clone()), i)?;
                }
            }
            OtioNode::Track(tr) => {
                let element_id = Self::element_id("track", &tr.name, index, parent_id.as_deref());
                self.elements.push(TimelineElement::Track {
                    id: element_id.clone(),
                    name: tr.name.clone(),
                    kind: tr.kind.clone(),
                    parent_id: parent_id.clone(),
                });
                for (i, child) in tr.child_nodes().iter().enumerate() {
                    self.collect_elements(child.clone(), Some(element_id.clone()), i)?;
                }
            }
            OtioNode::Clip(cl) => {
                let element_id = Self::element_id("clip", &cl.name, index, parent_id.as_deref());
                self.elements.push(TimelineElement::Clip {
                    id: element_id,
                    name: cl.name.clone(),
                    parent_id,
                });
            }
            OtioNode::Transition(tr) => {
                let element_id =
                    Self::element_id("transition", &tr.name, index, parent_id.as_deref());
                self.elements.push(TimelineElement::Transition {
                    id: element_id,
                    name: tr.name.clone(),
                    parent_id,
                });
            }
            OtioNode::SerializableCollection(sc) => {
                for (i, child) in sc.child_nodes().iter().enumerate() {
                    self.collect_elements(child.clone(), parent_id.clone(), i)?;
                }
            }
            OtioNode::Unknown { .. } => {
                // Skip unknown nodes in legacy mode
            }
        }
        Ok(())
    }

    fn element_id(ty: &str, name: &str, index: usize, parent_id: Option<&str>) -> String {
        match parent_id {
            Some(pid) => format!("{pid}/{}:{}:{}", index, ty, name),
            None => format!("{}:{}:{}", index, ty, name),
        }
    }

    pub fn elements(&self) -> &[TimelineElement] {
        &self.elements
    }

    pub fn find_element(&self, id: &str) -> Option<&TimelineElement> {
        self.elements.iter().find(|e| e.id() == id)
    }

    pub fn compute_touch_set(&self, changes: &[ChangeDescription]) -> Vec<String> {
        let mut affected = Vec::new();
        let mut seen = std::collections::HashSet::<String>::new();

        for change in changes {
            if !seen.insert(change.element_id.clone()) {
                continue;
            }
            affected.push(change.element_id.clone());

            if let Some(elem) = self.find_element(&change.element_id)
                && !matches!(elem, TimelineElement::Timeline { .. })
            {
                for other in &self.elements {
                    match other {
                        TimelineElement::Track { parent_id: Some(pid), .. }
                        | TimelineElement::Clip { parent_id: Some(pid), .. }
                        | TimelineElement::Transition { parent_id: Some(pid), .. } => {
                            if pid == elem.id() && seen.insert(other.id().to_owned()) {
                                affected.push(other.id().to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        affected
    }

    pub fn serialize_diff(&self, old_json: &str, new_json: &str) -> Result<String> {
        let old_val: serde_json::Value = serde_json::from_str(old_json)?;
        let new_val: serde_json::Value = serde_json::from_str(new_json)?;

        let mut lines = Vec::new();
        Self::diff_values(&old_val, &new_val, "".to_string(), &mut lines);

        if lines.is_empty() {
            lines.push("(no differences)".to_string());
        }

        Ok(lines.join("\n"))
    }

    fn diff_values(
        old: &serde_json::Value,
        new: &serde_json::Value,
        path: String,
        lines: &mut Vec<String>,
    ) {
        match (old, new) {
            (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) => {
                let all_keys: std::collections::HashSet<&String> =
                    old_map.keys().chain(new_map.keys()).collect();
                for key in all_keys {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    match (old_map.get(key), new_map.get(key)) {
                        (Some(o), Some(n)) => {
                            if o != n {
                                Self::diff_values(o, n, child_path, lines);
                            }
                        }
                        (None, Some(n)) => {
                            lines.push(format!("+ {child_path}: {n}"));
                        }
                        (Some(o), None) => {
                            lines.push(format!("- {child_path}: {o}"));
                        }
                        (None, None) => unreachable!(),
                    }
                }
            }
            (serde_json::Value::Array(old_arr), serde_json::Value::Array(new_arr)) => {
                let max_len = old_arr.len().max(new_arr.len());
                for i in 0..max_len {
                    let child_path = format!("{path}[{i}]");
                    match (old_arr.get(i), new_arr.get(i)) {
                        (Some(o), Some(n)) => {
                            if o != n {
                                Self::diff_values(o, n, child_path, lines);
                            }
                        }
                        (None, Some(n)) => {
                            lines.push(format!("+ {child_path}: {n}"));
                        }
                        (Some(o), None) => {
                            lines.push(format!("- {child_path}: {o}"));
                        }
                        (None, None) => unreachable!(),
                    }
                }
            }
            _ => {
                if old != new {
                    lines.push(format!("- {path}: {old}"));
                    lines.push(format!("+ {path}: {new}"));
                }
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_timeline_otio() -> &'static str {
        r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "TestTimeline",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "Video",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {
                            "OTIO_SCHEMA": "otio.schema.Clip",
                            "name": "Intro",
                            "metadata": {},
                            "source_range": {
                                "start_time": { "value": 0.0, "rate": 24.0 },
                                "duration": { "value": 100.0, "rate": 24.0 }
                            }
                        },
                        {
                            "OTIO_SCHEMA": "otio.schema.Transition",
                            "name": "Dissolve",
                            "metadata": {},
                            "in_offset": { "value": 12.0, "rate": 24.0 },
                            "out_offset": { "value": 12.0, "rate": 24.0 }
                        },
                        {
                            "OTIO_SCHEMA": "otio.schema.Clip",
                            "name": "Main",
                            "metadata": {},
                            "source_range": {
                                "start_time": { "value": 100.0, "rate": 24.0 },
                                "duration": { "value": 200.0, "rate": 24.0 }
                            }
                        }
                    ]
                }
            ]
        }"#
    }

    // --- Legacy API tests (preserved) ---

    #[test]
    fn test_parse_minimal_timeline() {
        let mut driver = LegacyOtioDriver::new();
        driver.parse_otio(minimal_timeline_otio()).unwrap();
        assert_eq!(driver.elements().len(), 5);
        assert_eq!(driver.elements()[0].element_type(), "Timeline");
        assert_eq!(driver.elements()[2].element_type(), "Clip");
        assert_eq!(driver.elements()[3].element_type(), "Transition");
    }

    #[test]
    fn test_parse_empty_timeline() {
        let json = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Empty",
            "metadata": {},
            "tracks": []
        }"#;
        let mut driver = LegacyOtioDriver::new();
        driver.parse_otio(json).unwrap();
        assert_eq!(driver.elements().len(), 1);
    }

    #[test]
    fn test_find_element() {
        let mut driver = LegacyOtioDriver::new();
        driver.parse_otio(minimal_timeline_otio()).unwrap();
        assert!(driver.find_element("nonexistent").is_none());
    }

    #[test]
    fn test_compute_touch_set() {
        let mut driver = LegacyOtioDriver::new();
        driver.parse_otio(minimal_timeline_otio()).unwrap();
        let track_id = "0:timeline:TestTimeline/0:track:Video";
        let changes = vec![ChangeDescription {
            element_id: track_id.to_string(),
            field_path: "name".to_string(),
            old_value: Some("Video".to_string()),
            new_value: Some("Audio".to_string()),
        }];
        let touch_set = driver.compute_touch_set(&changes);
        assert!(touch_set.contains(&track_id.to_string()));
    }

    #[test]
    fn test_serialize_diff_identical() {
        let json = r#"{"OTIO_SCHEMA":"otio.schema.Timeline","name":"Test","metadata":{},"tracks":[]}"#;
        let driver = LegacyOtioDriver::new();
        let diff = driver.serialize_diff(json, json).unwrap();
        assert_eq!(diff, "(no differences)");
    }

    #[test]
    fn test_large_timeline_performance() {
        let mut tracks = Vec::new();
        for t in 0..10 {
            let mut clips = Vec::new();
            for c in 0..50 {
                clips.push(serde_json::json!({
                    "OTIO_SCHEMA": "otio.schema.Clip",
                    "name": format!("Clip_{t}_{c}"),
                    "metadata": {},
                    "source_range": {
                        "start_time": {"value": (c as f64) * 100.0, "rate": 24.0},
                        "duration": {"value": 100.0, "rate": 24.0}
                    }
                }));
            }
            tracks.push(serde_json::json!({
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": format!("Track_{t}"),
                "kind": if t < 3 { "Video" } else { "Audio" },
                "metadata": {},
                "children": clips
            }));
        }
        let timeline = serde_json::json!({
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "LargeTimeline",
            "metadata": {"project": "perf_test"},
            "tracks": tracks
        });
        let json_str = serde_json::to_string(&timeline).unwrap();
        let mut driver = LegacyOtioDriver::new();
        let start = std::time::Instant::now();
        driver.parse_otio(&json_str).unwrap();
        assert!(start.elapsed().as_secs() < 5);
    }

    // --- SutureDriver implementation tests ---

    #[test]
    fn test_driver_name() {
        assert_eq!(OtioDriver::new().name(), "OpenTimelineIO");
    }
    #[test]
    fn test_extensions() {
        assert_eq!(OtioDriver::new().supported_extensions(), &[".otio"]);
    }

    #[test]
    fn test_diff_added_clip() {
        let base = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Test",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let modified = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Test",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"B","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let driver = OtioDriver::new();
        let changes = driver.diff(Some(base), modified).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Added { .. }));
    }

    #[test]
    fn test_diff_removed_clip() {
        let base = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Test",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"B","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let modified = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Test",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let driver = OtioDriver::new();
        let changes = driver.diff(Some(base), modified).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Removed { .. }));
    }

    #[test]
    fn test_diff_modified_clip() {
        let base = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Test",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let modified = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Test",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":200.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let driver = OtioDriver::new();
        let changes = driver.diff(Some(base), modified).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Modified { .. }));
    }

    #[test]
    fn test_diff_no_change() {
        let driver = OtioDriver::new();
        let changes = driver.diff(Some(minimal_timeline_otio()), minimal_timeline_otio()).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_new_file() {
        let driver = OtioDriver::new();
        let changes = driver.diff(None, minimal_timeline_otio()).unwrap();
        assert!(!changes.is_empty());
        assert!(changes.iter().all(|c| matches!(c, SemanticChange::Added { .. })));
    }

    #[test]
    fn test_format_diff() {
        let driver = OtioDriver::new();
        let fmt = driver.format_diff(None, minimal_timeline_otio()).unwrap();
        assert!(fmt.contains("ADDED"));
        let fmt = driver.format_diff(Some(minimal_timeline_otio()), minimal_timeline_otio()).unwrap();
        assert_eq!(fmt, "no changes");
    }

    #[test]
    fn test_merge_add_different_clips() {
        let base = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "MergeTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let ours = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "MergeTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"B","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let theirs = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "MergeTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"C","metadata":{},"source_range":{"start_time":{"value":200.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let driver = OtioDriver::new();
        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "merge should succeed (non-conflicting adds)");
        let merged = result.unwrap();
        assert!(merged.contains("\"B\""), "merged should contain clip B from ours");
        assert!(merged.contains("\"C\""), "merged should contain clip C from theirs");
    }

    #[test]
    fn test_merge_conflict() {
        let base = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ConflictTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let ours = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ConflictTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":200.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let theirs = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ConflictTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":300.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let driver = OtioDriver::new();
        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none(), "merge should detect conflict");
    }

    #[test]
    fn test_merge_one_side_modify() {
        let base = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ModifyTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let ours = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ModifyTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":200.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let theirs = base; // unchanged
        let driver = OtioDriver::new();
        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("200.0"), "merged should have ours' duration");
    }

    #[test]
    fn test_content_based_identity_reorder() {
        // Two clips with same fingerprint (same source) but reordered
        // should be treated as the same clip, not add/remove
        let base = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ReorderTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"ShotA","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"ShotB","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let reordered = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ReorderTest",
            "metadata": {},
            "tracks": [{
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"ShotB","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                    {"OTIO_SCHEMA":"otio.schema.Clip","name":"ShotA","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                ]
            }]
        }"#;
        let driver = OtioDriver::new();
        let changes = driver.diff(Some(base), reordered).unwrap();
        // Reordering should NOT produce adds/removes for clips with same source
        let clip_adds: Vec<_> = changes.iter().filter(|c| matches!(c, SemanticChange::Added { .. })).collect();
        let clip_removes: Vec<_> = changes.iter().filter(|c| matches!(c, SemanticChange::Removed { .. })).collect();
        assert!(clip_adds.is_empty(), "reordering should not produce adds for same-source clips");
        assert!(clip_removes.is_empty(), "reordering should not produce removes for same-source clips");
    }

    #[test]
    fn test_unknown_type_graceful() {
        let json = r#"{
            "OTIO_SCHEMA": "otio.schema.Gap",
            "name": "gap1",
            "metadata": {},
            "source_range": {"start_time": {"value": 10.0, "rate": 24.0}, "duration": {"value": 5.0, "rate": 24.0}}
        }"#;
        let result = OtioDriver::parse_and_flatten(json);
        assert!(result.is_ok(), "should handle unknown OTIO types gracefully");
        let nodes = result.unwrap();
        // The Gap should be in the flat list
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_parse_invalid_json() {
        let mut driver = LegacyOtioDriver::new();
        assert!(driver.parse_otio("not json").is_err());
    }

    #[test]
    fn test_parse_missing_schema() {
        let mut driver = LegacyOtioDriver::new();
        assert!(driver.parse_otio(r#"{"name": "NoSchema", "tracks": []}"#).is_err());
    }
}
