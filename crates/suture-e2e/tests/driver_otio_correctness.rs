use suture_driver_otio::{ChangeDescription, OtioDriver};

fn multi_track_timeline() -> &'static str {
    r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "MultiTrack",
        "metadata": {},
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Video1",
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
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Middle",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 100.0, "rate": 24.0 },
                            "duration": { "value": 150.0, "rate": 24.0 }
                        }
                    }
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Audio1",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "BG_Music",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 48.0 },
                            "duration": { "value": 500.0, "rate": 48.0 }
                        }
                    }
                ]
            }
        ]
    }"#
}

fn modified_clip_timeline() -> &'static str {
    r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "MultiTrack",
        "metadata": {},
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Video1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Opening",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 24.0 },
                            "duration": { "value": 50.0, "rate": 24.0 }
                        }
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Middle",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 100.0, "rate": 24.0 },
                            "duration": { "value": 150.0, "rate": 24.0 }
                        }
                    }
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Audio1",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "BG_Music",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 48.0 },
                            "duration": { "value": 500.0, "rate": 48.0 }
                        }
                    }
                ]
            }
        ]
    }"#
}

fn added_audio_track_timeline() -> &'static str {
    r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "MultiTrack",
        "metadata": {},
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Video1",
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
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Middle",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 100.0, "rate": 24.0 },
                            "duration": { "value": 150.0, "rate": 24.0 }
                        }
                    }
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Audio1",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "BG_Music",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 48.0 },
                            "duration": { "value": 500.0, "rate": 48.0 }
                        }
                    }
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Audio2",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "Dialogue",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 48.0 },
                            "duration": { "value": 250.0, "rate": 48.0 }
                        }
                    }
                ]
            }
        ]
    }"#
}

#[test]
fn otio_parse_multi_track_timeline() {
    let mut driver = OtioDriver::new();
    driver.parse_otio(multi_track_timeline()).unwrap();

    let elements = driver.elements();
    assert_eq!(
        elements.len(),
        6,
        "should have 6 elements: 1 timeline + 2 tracks + 3 clips"
    );

    assert_eq!(elements[0].element_type(), "Timeline");
    assert_eq!(elements[0].name(), "MultiTrack");

    let video_track = &elements[1];
    assert_eq!(video_track.element_type(), "Track");
    assert_eq!(video_track.name(), "Video1");

    let audio_track = &elements[4];
    assert_eq!(audio_track.element_type(), "Track");
    assert_eq!(audio_track.name(), "Audio1");

    let clips: Vec<_> = elements
        .iter()
        .filter(|e| e.element_type() == "Clip")
        .collect();
    assert_eq!(clips.len(), 3);
}

#[test]
fn otio_diff_detects_clip_trim() {
    let driver = OtioDriver::new();
    let diff = driver
        .serialize_diff(multi_track_timeline(), modified_clip_timeline())
        .unwrap();

    assert!(
        diff.contains("50.0"),
        "diff should detect the trimmed duration change"
    );
    assert!(
        diff.contains("100.0"),
        "diff should show the old duration value"
    );
}

#[test]
fn otio_diff_detects_added_track() {
    let driver = OtioDriver::new();
    let diff = driver
        .serialize_diff(multi_track_timeline(), added_audio_track_timeline())
        .unwrap();

    assert!(
        diff.contains("Audio2"),
        "diff should detect the added audio track"
    );
    assert!(
        diff.contains("Dialogue"),
        "diff should detect the added dialogue clip"
    );
}

#[test]
fn otio_touch_set_cascades_from_track_to_children() {
    let mut driver = OtioDriver::new();
    driver.parse_otio(multi_track_timeline()).unwrap();

    let track_id = "0:timeline:MultiTrack/0:track:Video1";
    let changes = vec![ChangeDescription {
        element_id: track_id.to_string(),
        field_path: "name".to_string(),
        old_value: Some("Video1".to_string()),
        new_value: Some("MainVideo".to_string()),
    }];

    let touch_set = driver.compute_touch_set(&changes);
    assert!(
        touch_set.contains(&track_id.to_string()),
        "touch set should contain the modified track"
    );
    assert!(
        touch_set.contains(&"0:timeline:MultiTrack/0:track:Video1/0:clip:Opening".to_string()),
        "touch set should cascade to child clip Opening"
    );
    assert!(
        touch_set.contains(&"0:timeline:MultiTrack/0:track:Video1/1:clip:Middle".to_string()),
        "touch set should cascade to child clip Middle"
    );
    assert!(
        !touch_set.contains(&"0:timeline:MultiTrack/1:track:Audio1".to_string()),
        "touch set should not cascade to unrelated track"
    );
}

#[test]
fn otio_touch_set_no_cascade_for_timeline() {
    let mut driver = OtioDriver::new();
    driver.parse_otio(multi_track_timeline()).unwrap();

    let changes = vec![ChangeDescription {
        element_id: "0:timeline:MultiTrack".to_string(),
        field_path: "name".to_string(),
        old_value: Some("MultiTrack".to_string()),
        new_value: Some("Renamed".to_string()),
    }];

    let touch_set = driver.compute_touch_set(&changes);
    assert_eq!(touch_set, vec!["0:timeline:MultiTrack"]);
}

#[test]
fn otio_diff_identical_timelines() {
    let driver = OtioDriver::new();
    let diff = driver
        .serialize_diff(multi_track_timeline(), multi_track_timeline())
        .unwrap();
    assert_eq!(diff, "(no differences)");
}

#[test]
fn otio_diff_detects_timeline_name_change() {
    let renamed = r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "RenamedTimeline",
        "metadata": {},
        "tracks": []
    }"#;

    let driver = OtioDriver::new();
    let diff = driver
        .serialize_diff(
            r#"{"OTIO_SCHEMA":"otio.schema.Timeline","name":"Original","metadata":{},"tracks":[]}"#,
            renamed,
        )
        .unwrap();

    assert!(diff.contains("RenamedTimeline"));
    assert!(diff.contains("Original"));
}

#[test]
fn otio_find_element_by_id() {
    let mut driver = OtioDriver::new();
    driver.parse_otio(multi_track_timeline()).unwrap();

    let opening_id = "0:timeline:MultiTrack/0:track:Video1/0:clip:Opening";
    let elem = driver.find_element(opening_id);
    assert!(elem.is_some(), "should find the Opening clip by ID");
    assert_eq!(elem.unwrap().name(), "Opening");

    assert!(driver.find_element("nonexistent").is_none());
}

#[test]
fn otio_parse_timeline_with_transition() {
    let json = r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "WithTransition",
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
                        "name": "A",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 0.0, "rate": 24.0 },
                            "duration": { "value": 100.0, "rate": 24.0 }
                        }
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Transition",
                        "name": "CrossDissolve",
                        "metadata": {},
                        "in_offset": { "value": 12.0, "rate": 24.0 },
                        "out_offset": { "value": 12.0, "rate": 24.0 }
                    },
                    {
                        "OTIO_SCHEMA": "otio.schema.Clip",
                        "name": "B",
                        "metadata": {},
                        "source_range": {
                            "start_time": { "value": 100.0, "rate": 24.0 },
                            "duration": { "value": 200.0, "rate": 24.0 }
                        }
                    }
                ]
            }
        ]
    }"#;

    let mut driver = OtioDriver::new();
    driver.parse_otio(json).unwrap();

    let transitions: Vec<_> = driver
        .elements()
        .iter()
        .filter(|e| e.element_type() == "Transition")
        .collect();
    assert_eq!(transitions.len(), 1, "should have one transition");
    assert_eq!(transitions[0].name(), "CrossDissolve");

    let clips: Vec<_> = driver
        .elements()
        .iter()
        .filter(|e| e.element_type() == "Clip")
        .collect();
    assert_eq!(clips.len(), 2);
}

#[test]
fn otio_touch_set_for_transition_parent() {
    let json = r#"{
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "T",
        "metadata": {},
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1",
                "kind": "Video",
                "metadata": {},
                "children": [
                    {
                        "OTIO_SCHEMA": "otio.schema.Transition",
                        "name": "Fade",
                        "metadata": {},
                        "in_offset": { "value": 10.0, "rate": 24.0 },
                        "out_offset": { "value": 10.0, "rate": 24.0 }
                    }
                ]
            }
        ]
    }"#;

    let mut driver = OtioDriver::new();
    driver.parse_otio(json).unwrap();

    let transition_id = "0:timeline:T/0:track:V1/0:transition:Fade";
    let changes = vec![ChangeDescription {
        element_id: transition_id.to_string(),
        field_path: "in_offset.value".to_string(),
        old_value: Some("10.0".to_string()),
        new_value: Some("20.0".to_string()),
    }];

    let touch_set = driver.compute_touch_set(&changes);
    assert!(
        touch_set.contains(&transition_id.to_string()),
        "touch set should contain the modified transition"
    );
}
