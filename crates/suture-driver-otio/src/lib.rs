use serde::{Deserialize, Serialize};
use thiserror::Error;

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
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "OTIO_SCHEMA")]
pub enum OtioNode {
    #[serde(rename = "otio.schema.Timeline")]
    Timeline(Timeline),

    #[serde(rename = "otio.schema.Track")]
    Track(Track),

    #[serde(rename = "otio.schema.Clip")]
    Clip(Clip),

    #[serde(rename = "otio.schema.Transition")]
    Transition(Transition),

    #[serde(rename = "otio.schema.SerializableCollection")]
    SerializableCollection(SerializableCollection),

    #[serde(rename = "otio.schema.Stack")]
    Stack(Stack),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub tracks: Vec<OtioNode>,
    pub global_start_time: Option<RationalTime>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(rename = "kind")]
    pub kind: String,
    #[serde(default)]
    pub children: Vec<OtioNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub children: Vec<OtioNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub source_range: Option<TimeRange>,
    #[serde(default)]
    pub media_reference: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub in_offset: Option<RationalTime>,
    #[serde(default)]
    pub out_offset: Option<RationalTime>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SerializableCollection {
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub children: Vec<OtioNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RationalTime {
    pub value: f64,
    pub rate: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_time: RationalTime,
    pub duration: RationalTime,
}

// =============================================================================
// Parsed Timeline Elements (flattened view)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub enum TimelineElement {
    Timeline {
        id: String,
        name: String,
    },
    Track {
        id: String,
        name: String,
        kind: String,
        parent_id: Option<String>,
    },
    Clip {
        id: String,
        name: String,
        parent_id: Option<String>,
    },
    Transition {
        id: String,
        name: String,
        parent_id: Option<String>,
    },
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

// =============================================================================
// Touch Set Computation
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct ChangeDescription {
    pub element_id: String,
    pub field_path: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

// =============================================================================
// OtioDriver
// =============================================================================

pub struct OtioDriver {
    elements: Vec<TimelineElement>,
    raw_json: serde_json::Value,
}

impl OtioDriver {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            raw_json: serde_json::Value::Null,
        }
    }

    pub fn parse_otio(&mut self, input: &str) -> Result<()> {
        let root: serde_json::Value = serde_json::from_str(input)?;
        self.raw_json = root.clone();

        let node: OtioNode = serde_json::from_value(root)
            .map_err(|e| OtioError::InvalidStructure(format!("root node parse error: {e}")))?;

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
                for (i, child) in tl.tracks.iter().enumerate() {
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
                for (i, child) in st.children.iter().enumerate() {
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
                for (i, child) in tr.children.iter().enumerate() {
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
                for (i, child) in sc.children.iter().enumerate() {
                    self.collect_elements(child.clone(), parent_id.clone(), i)?;
                }
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
                        TimelineElement::Track {
                            parent_id: Some(pid),
                            ..
                        }
                        | TimelineElement::Clip {
                            parent_id: Some(pid),
                            ..
                        }
                        | TimelineElement::Transition {
                            parent_id: Some(pid),
                            ..
                        } => {
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

impl Default for OtioDriver {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_parse_minimal_timeline() {
        let mut driver = OtioDriver::new();
        driver.parse_otio(minimal_timeline_otio()).unwrap();

        assert_eq!(driver.elements().len(), 5);

        assert_eq!(driver.elements()[0].element_type(), "Timeline");
        assert_eq!(driver.elements()[0].name(), "TestTimeline");

        assert_eq!(driver.elements()[1].element_type(), "Track");
        assert_eq!(driver.elements()[1].name(), "Video");

        assert_eq!(driver.elements()[2].element_type(), "Clip");
        assert_eq!(driver.elements()[2].name(), "Intro");

        assert_eq!(driver.elements()[3].element_type(), "Transition");
        assert_eq!(driver.elements()[3].name(), "Dissolve");

        assert_eq!(driver.elements()[4].element_type(), "Clip");
        assert_eq!(driver.elements()[4].name(), "Main");
    }

    #[test]
    fn test_parse_empty_timeline() {
        let json = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Empty",
            "metadata": {},
            "tracks": []
        }"#;

        let mut driver = OtioDriver::new();
        driver.parse_otio(json).unwrap();
        assert_eq!(driver.elements().len(), 1);
        assert_eq!(driver.elements()[0].name(), "Empty");
    }

    #[test]
    fn test_find_element() {
        let mut driver = OtioDriver::new();
        driver.parse_otio(minimal_timeline_otio()).unwrap();

        let tl = driver.find_element("0:timeline:TestTimeline").unwrap();
        assert_eq!(tl.name(), "TestTimeline");

        let clip = driver
            .find_element("0:timeline:TestTimeline/0:track:Video/0:clip:Intro")
            .unwrap();
        assert_eq!(clip.name(), "Intro");

        assert!(driver.find_element("nonexistent").is_none());
    }

    #[test]
    fn test_compute_touch_set() {
        let mut driver = OtioDriver::new();
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
        assert!(
            touch_set.contains(&"0:timeline:TestTimeline/0:track:Video/0:clip:Intro".to_string())
        );
        assert!(
            touch_set.contains(
                &"0:timeline:TestTimeline/0:track:Video/1:transition:Dissolve".to_string()
            )
        );
        assert!(
            touch_set.contains(&"0:timeline:TestTimeline/0:track:Video/2:clip:Main".to_string())
        );
    }

    #[test]
    fn test_compute_touch_set_no_cascade() {
        let mut driver = OtioDriver::new();
        driver.parse_otio(minimal_timeline_otio()).unwrap();

        let changes = vec![ChangeDescription {
            element_id: "0:timeline:TestTimeline".to_string(),
            field_path: "name".to_string(),
            old_value: Some("TestTimeline".to_string()),
            new_value: Some("NewName".to_string()),
        }];

        let touch_set = driver.compute_touch_set(&changes);
        assert_eq!(touch_set, vec!["0:timeline:TestTimeline"]);
    }

    #[test]
    fn test_serialize_diff_added_field() {
        let old =
            r#"{"OTIO_SCHEMA":"otio.schema.Timeline","name":"Test","metadata":{},"tracks":[]}"#;
        let new = r#"{"OTIO_SCHEMA":"otio.schema.Timeline","name":"Test","metadata":{},"tracks":[],"global_start_time":{"value":0.0,"rate":24.0}}"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();

        assert!(diff.contains("+ global_start_time"));
    }

    #[test]
    fn test_serialize_diff_name_change() {
        let old =
            r#"{"OTIO_SCHEMA":"otio.schema.Timeline","name":"Old","metadata":{},"tracks":[]}"#;
        let new =
            r#"{"OTIO_SCHEMA":"otio.schema.Timeline","name":"New","metadata":{},"tracks":[]}"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();

        assert!(diff.contains(&"- name: \"Old\"".to_string()));
        assert!(diff.contains(&"+ name: \"New\"".to_string()));
    }

    #[test]
    fn test_serialize_diff_identical() {
        let json =
            r#"{"OTIO_SCHEMA":"otio.schema.Timeline","name":"Test","metadata":{},"tracks":[]}"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(json, json).unwrap();

        assert_eq!(diff, "(no differences)");
    }

    #[test]
    fn test_parse_invalid_json() {
        let mut driver = OtioDriver::new();
        let result = driver.parse_otio("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_schema() {
        let mut driver = OtioDriver::new();
        let result = driver.parse_otio(r#"{"name": "NoSchema", "tracks": []}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_timeline_element_type_and_name() {
        let elem = TimelineElement::Clip {
            id: "test_clip".to_string(),
            name: "MyClip".to_string(),
            parent_id: None,
        };
        assert_eq!(elem.element_type(), "Clip");
        assert_eq!(elem.name(), "MyClip");
        assert_eq!(elem.id(), "test_clip");
    }

    #[test]
    fn test_nested_tracks() {
        let json = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Multi",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": []
                },
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "A1",
                    "kind": "Audio",
                    "metadata": {},
                    "children": []
                }
            ]
        }"#;

        let mut driver = OtioDriver::new();
        driver.parse_otio(json).unwrap();
        assert_eq!(driver.elements().len(), 3);
        assert_eq!(driver.elements()[1].name(), "V1");
        assert_eq!(driver.elements()[2].name(), "A1");
    }

    #[test]
    fn test_unique_ids_for_same_type() {
        let json = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "Test",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {
                            "OTIO_SCHEMA": "otio.schema.Clip",
                            "name": "Shot",
                            "metadata": {},
                            "source_range": {
                                "start_time": { "value": 0.0, "rate": 24.0 },
                                "duration": { "value": 100.0, "rate": 24.0 }
                            }
                        },
                        {
                            "OTIO_SCHEMA": "otio.schema.Clip",
                            "name": "Shot",
                            "metadata": {},
                            "source_range": {
                                "start_time": { "value": 100.0, "rate": 24.0 },
                                "duration": { "value": 100.0, "rate": 24.0 }
                            }
                        }
                    ]
                }
            ]
        }"#;

        let mut driver = OtioDriver::new();
        driver.parse_otio(json).unwrap();

        let clip_ids: Vec<&str> = driver
            .elements()
            .iter()
            .filter(|e| e.element_type() == "Clip")
            .map(|e| e.id())
            .collect();
        assert_eq!(clip_ids.len(), 2);
        assert_ne!(clip_ids[0], clip_ids[1]);
    }

    #[test]
    fn test_merge_clip_reordering() {
        let old = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ReorderTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"B","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"C","metadata":{},"source_range":{"start_time":{"value":200.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"D","metadata":{},"source_range":{"start_time":{"value":300.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"E","metadata":{},"source_range":{"start_time":{"value":400.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                    ]
                }
            ]
        }"#;

        let new = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "ReorderTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"A","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"C","metadata":{},"source_range":{"start_time":{"value":200.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"B","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"D","metadata":{},"source_range":{"start_time":{"value":300.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"E","metadata":{},"source_range":{"start_time":{"value":400.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                    ]
                }
            ]
        }"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();
        assert_ne!(diff, "(no differences)", "diff should detect clip reordering");
        assert!(diff.contains("B"), "diff should reference moved clip B");
        assert!(diff.contains("C"), "diff should reference moved clip C");
    }

    #[test]
    fn test_merge_transition_addition() {
        let old = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "TransTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"Clip1","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"Clip2","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":200.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"Clip3","metadata":{},"source_range":{"start_time":{"value":300.0,"rate":24.0},"duration":{"value":150.0,"rate":24.0}}}
                    ]
                }
            ]
        }"#;

        let new = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "TransTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"Clip1","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Transition","name":"CrossDissolve","metadata":{},"in_offset":{"value":12.0,"rate":24.0},"out_offset":{"value":12.0,"rate":24.0}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"Clip2","metadata":{},"source_range":{"start_time":{"value":100.0,"rate":24.0},"duration":{"value":200.0,"rate":24.0}}},
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"Clip3","metadata":{},"source_range":{"start_time":{"value":300.0,"rate":24.0},"duration":{"value":150.0,"rate":24.0}}}
                    ]
                }
            ]
        }"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();
        assert!(
            diff.contains("CrossDissolve"),
            "diff should detect new transition"
        );
    }

    #[test]
    fn test_merge_duration_change() {
        let old = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "DurTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"ClipA","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                    ]
                }
            ]
        }"#;

        let new = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "DurTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": [
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"ClipA","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":200.0,"rate":24.0}}}
                    ]
                }
            ]
        }"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();
        assert!(
            diff.contains("100") && diff.contains("200"),
            "diff should show duration change from 100 to 200"
        );
    }

    #[test]
    fn test_merge_track_addition() {
        let old = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "TrackTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": []
                },
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "A1",
                    "kind": "Audio",
                    "metadata": {},
                    "children": []
                }
            ]
        }"#;

        let new = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "TrackTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "V1",
                    "kind": "Video",
                    "metadata": {},
                    "children": []
                },
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "A1",
                    "kind": "Audio",
                    "metadata": {},
                    "children": []
                },
                {
                    "OTIO_SCHEMA": "otio.schema.Track",
                    "name": "A2_Music",
                    "kind": "Audio",
                    "metadata": {},
                    "children": [
                        {"OTIO_SCHEMA":"otio.schema.Clip","name":"Score","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":48.0},"duration":{"value":500.0,"rate":48.0}}}
                    ]
                }
            ]
        }"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();
        assert!(diff.contains("A2_Music"), "diff should detect new track");
        assert!(diff.contains("Score"), "diff should detect clip in new track");
    }

    #[test]
    fn test_merge_metadata_change() {
        let old = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "MetaTest",
            "metadata": {"project": "test"},
            "tracks": []
        }"#;

        let new = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "MetaTest",
            "metadata": {"project": "test", "color_space": "sRGB"},
            "tracks": []
        }"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();
        assert!(
            diff.contains("color_space"),
            "diff should show new metadata field"
        );
        assert!(diff.contains("sRGB"), "diff should show sRGB value");
    }

    #[test]
    fn test_large_timeline_performance() {
        use std::time::Instant;

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

        let mut driver = OtioDriver::new();
        let start = Instant::now();
        driver.parse_otio(&json_str).unwrap();
        let parse_time = start.elapsed();

        assert_eq!(
            driver.elements().len(),
            511,
            "should have 511 elements (1 timeline + 10 tracks + 500 clips)"
        );
        assert!(
            parse_time.as_secs() < 5,
            "parsing 500 clips should complete in under 5 seconds, took {:?}",
            parse_time
        );

        let track_id = driver.elements()[1].id().to_string();
        let changes = vec![ChangeDescription {
            element_id: track_id.clone(),
            field_path: "name".to_string(),
            old_value: Some("Track_0".to_string()),
            new_value: Some("Track_0_Renamed".to_string()),
        }];

        let start = Instant::now();
        let touch_set = driver.compute_touch_set(&changes);
        let touch_time = start.elapsed();
        assert!(!touch_set.is_empty(), "touch set should not be empty");
        assert!(
            touch_time.as_secs() < 5,
            "compute_touch_set should complete in under 5 seconds, took {:?}",
            touch_time
        );

        let start = Instant::now();
        let diff = driver.serialize_diff(&json_str, &json_str).unwrap();
        let diff_time = start.elapsed();
        assert_eq!(diff, "(no differences)");
        assert!(
            diff_time.as_secs() < 5,
            "serialize_diff should complete in under 5 seconds, took {:?}",
            diff_time
        );
    }

    #[test]
    fn test_merge_nested_stack() {
        let old = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "StackTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Stack",
                    "name": "MainStack",
                    "metadata": {},
                    "children": [
                        {
                            "OTIO_SCHEMA": "otio.schema.Track",
                            "name": "InnerTrack",
                            "kind": "Video",
                            "metadata": {},
                            "children": [
                                {"OTIO_SCHEMA":"otio.schema.Clip","name":"DeepClip","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":100.0,"rate":24.0}}}
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let new = r#"{
            "OTIO_SCHEMA": "otio.schema.Timeline",
            "name": "StackTest",
            "metadata": {},
            "tracks": [
                {
                    "OTIO_SCHEMA": "otio.schema.Stack",
                    "name": "MainStack",
                    "metadata": {},
                    "children": [
                        {
                            "OTIO_SCHEMA": "otio.schema.Track",
                            "name": "InnerTrack",
                            "kind": "Video",
                            "metadata": {},
                            "children": [
                                {"OTIO_SCHEMA":"otio.schema.Clip","name":"DeepClip_MODIFIED","metadata":{},"source_range":{"start_time":{"value":0.0,"rate":24.0},"duration":{"value":200.0,"rate":24.0}}}
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let driver = OtioDriver::new();
        let diff = driver.serialize_diff(old, new).unwrap();
        assert_ne!(
            diff, "(no differences)",
            "diff should detect deep change in nested stack"
        );
        assert!(
            diff.contains("DeepClip_MODIFIED"),
            "diff should show modified clip name"
        );

        let mut driver = OtioDriver::new();
        driver.parse_otio(new).unwrap();
        let stacks: Vec<_> = driver
            .elements()
            .iter()
            .filter(|e| e.element_type() == "Track" && e.name() == "MainStack")
            .collect();
        assert_eq!(stacks.len(), 1, "should find the Stack element");
        assert_eq!(driver.elements().len(), 4, "timeline + stack + inner track + deep clip");
    }
}
