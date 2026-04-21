use suture_driver_otio::{ChangeDescription, LegacyOtioDriver};

#[test]
fn otio_realistic_simple_parse() {
    let mut driver = LegacyOtioDriver::new();
    let json = suture_e2e::fixtures::otio::simple();
    driver.parse_otio(&json).unwrap();

    let elements = driver.elements();
    assert!(
        elements.len() >= 6,
        "simple timeline should have at least 6 elements, got {}",
        elements.len()
    );

    let tracks: Vec<_> = elements
        .iter()
        .filter(|e| e.element_type() == "Track")
        .collect();
    assert_eq!(tracks.len(), 2, "should have 2 tracks");

    let clips: Vec<_> = elements
        .iter()
        .filter(|e| e.element_type() == "Clip")
        .collect();
    assert_eq!(clips.len(), 6, "should have 6 clips");
}

#[test]
fn otio_realistic_simple_diff_clip_change() {
    let driver = LegacyOtioDriver::new();
    let base = suture_e2e::fixtures::otio::simple();

    let modified = r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "SimpleEdit",
        "metadata": {"project": "demo"},
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Opening_MODIFIED",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 24.0 },
                            "duration": { "value": 100.0, "rate": 24.0 }
                        }
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Interview_A",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 100.0, "rate": 24.0 },
                            "duration": { "value": 200.0, "rate": 24.0 }
                        }
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "B_Roll",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 300.0, "rate": 24.0 },
                            "duration": { "value": 50.0, "rate": 24.0 }
                        }
                    }
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "A1",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Ambient",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 48.0 },
                            "duration": { "value": 350.0, "rate": 48.0 }
                        }
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Music_Cue",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 50.0, "rate": 48.0 },
                            "duration": { "value": 120.0, "rate": 48.0 }
                        }
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Dialogue_A",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 100.0, "rate": 48.0 },
                            "duration": { "value": 180.0, "rate": 48.0 }
                        }
                    }
                ]
            }
        ]
    }"#;

    let diff = driver.serialize_diff(&base, modified).unwrap();
    assert!(
        diff.contains("Opening_MODIFIED"),
        "diff should detect clip name change"
    );
    assert!(diff.contains("Opening"), "diff should show old clip name");
}

#[test]
fn otio_realistic_complex_parse() {
    let mut driver = LegacyOtioDriver::new();
    let json = suture_e2e::fixtures::otio::complex();
    driver.parse_otio(&json).unwrap();

    let elements = driver.elements();
    assert!(
        elements.len() >= 15,
        "complex timeline should have many elements, got {}",
        elements.len()
    );

    let tracks: Vec<_> = elements
        .iter()
        .filter(|e| e.element_type() == "Track")
        .collect();
    assert!(
        tracks.len() >= 5,
        "should have at least 5 tracks, got {}",
        tracks.len()
    );

    let transitions: Vec<_> = elements
        .iter()
        .filter(|e| e.element_type() == "Transition")
        .collect();
    assert!(
        transitions.len() >= 2,
        "should have at least 2 transitions, got {}",
        transitions.len()
    );
}

#[test]
fn otio_realistic_complex_touch_set() {
    let mut driver = LegacyOtioDriver::new();
    let json = suture_e2e::fixtures::otio::complex();
    driver.parse_otio(&json).unwrap();

    let v1_track_id = "0:timeline:FeatureFilm_RoughCut/0:track:V1_Main";
    let changes = vec![ChangeDescription {
        element_id: v1_track_id.to_string(),
        field_path: "name".to_string(),
        old_value: Some("V1_Main".to_string()),
        new_value: Some("V1_Main_Renamed".to_string()),
    }];

    let touch_set = driver.compute_touch_set(&changes);
    assert!(
        touch_set.contains(&v1_track_id.to_string()),
        "touch set should contain the modified track"
    );

    let child_ids: Vec<_> = touch_set
        .iter()
        .filter(|id| id.starts_with(&format!("{v1_track_id}/")))
        .collect();
    assert!(
        !child_ids.is_empty(),
        "touch set should cascade to children of V1_Main"
    );
}

#[test]
fn otio_realistic_nested_parse() {
    let mut driver = LegacyOtioDriver::new();
    let json = suture_e2e::fixtures::otio::nested();
    driver.parse_otio(&json).unwrap();

    let elements = driver.elements();
    assert!(
        elements.len() >= 10,
        "nested timeline should have many elements, got {}",
        elements.len()
    );

    let stacks: Vec<_> = elements
        .iter()
        .filter(|e| e.element_type() == "Track" && e.name().contains("Nested"))
        .collect();
    assert!(!stacks.is_empty(), "should find nested track element");
}

#[test]
fn otio_realistic_diff_identical() {
    let driver = LegacyOtioDriver::new();
    let json = suture_e2e::fixtures::otio::complex();

    let diff = driver.serialize_diff(&json, &json).unwrap();
    assert_eq!(diff, "(no differences)");
}

#[test]
fn otio_realistic_complex_diff_track_addition() {
    let driver = LegacyOtioDriver::new();
    let base = suture_e2e::fixtures::otio::simple();

    let with_extra = r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "SimpleEdit",
        "metadata": {"project": "demo"},
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Opening",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 24.0 },
                            "duration": { "value": 100.0, "rate": 24.0 }
                        }
                    }
                ]
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
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Score",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 48.0 },
                            "duration": { "value": 500.0, "rate": 48.0 }
                        }
                    }
                ]
            }
        ]
    }"#;

    let diff = driver.serialize_diff(&base, with_extra).unwrap();
    assert!(
        diff.contains("A2_Music"),
        "diff should detect added music track"
    );
    assert!(
        diff.contains("Score"),
        "diff should detect added Score clip"
    );
}

#[test]
fn otio_multi_editor_merge_conflict() {
    let base_json = suture_e2e::fixtures::otio::complex();

    let base_val: serde_json::Value = serde_json::from_str(&base_json).unwrap();

    let mut editor_a_val = base_val.clone();
    if let Some(tracks) = editor_a_val
        .get_mut("tracks")
        .and_then(|t| t.as_array_mut())
        .and_then(|arr| arr.get_mut(0))
        .and_then(|t| t.get_mut("children"))
        .and_then(|c| c.as_array_mut())
    {
        for child in tracks.iter_mut() {
            if child.get("OTIO_SCHEMA").and_then(|s| s.as_str()) == Some("otio.schema.Clip") {
                if let Some(name) = child.get_mut("name").and_then(|n| n.as_str()) {
                    *child.get_mut("name").unwrap() = serde_json::json!(format!("{name}_EditorA"));
                }
            }
        }
    }
    let editor_a_json = serde_json::to_string(&editor_a_val).unwrap();

    let mut editor_b_val = base_val.clone();
    if let Some(tracks) = editor_b_val
        .get_mut("tracks")
        .and_then(|t| t.as_array_mut())
        .and_then(|arr| arr.get_mut(0))
        .and_then(|t| t.get_mut("children"))
        .and_then(|c| c.as_array_mut())
    {
        for child in tracks.iter_mut() {
            if let Some(dur) = child
                .get_mut("source_range")
                .and_then(|s| s.as_object_mut())
                .and_then(|o| o.get_mut("duration"))
                .and_then(|d| d.as_object_mut())
                .and_then(|o| o.get_mut("value"))
                .and_then(|v| v.as_f64())
            {
                child["source_range"]["duration"]["value"] = serde_json::json!(dur * 1.5);
            }
        }
    }
    let editor_b_json = serde_json::to_string(&editor_b_val).unwrap();

    let driver = LegacyOtioDriver::new();
    let diff = driver
        .serialize_diff(&editor_a_json, &editor_b_json)
        .unwrap();
    assert_ne!(
        diff, "(no differences)",
        "merge should detect conflicts between editors"
    );

    let mut driver_a = LegacyOtioDriver::new();
    driver_a.parse_otio(&editor_a_json).unwrap();
    assert!(
        driver_a.elements().len() >= 15,
        "editor A's timeline should parse with many elements"
    );

    let mut driver_b = LegacyOtioDriver::new();
    driver_b.parse_otio(&editor_b_json).unwrap();
    assert!(
        driver_b.elements().len() >= 15,
        "editor B's timeline should parse with many elements"
    );

    let a_val: serde_json::Value = serde_json::from_str(&editor_a_json).unwrap();
    let b_val: serde_json::Value = serde_json::from_str(&editor_b_json).unwrap();
    assert_eq!(
        a_val.get("OTIO_SCHEMA").unwrap().as_str(),
        Some("otio.schema.Timeline"),
        "editor A output must have valid OTIO_SCHEMA"
    );
    assert_eq!(
        b_val.get("OTIO_SCHEMA").unwrap().as_str(),
        Some("otio.schema.Timeline"),
        "editor B output must have valid OTIO_SCHEMA"
    );
}
