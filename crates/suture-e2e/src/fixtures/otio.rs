use serde_json::{Value, json};

pub fn simple() -> String {
    json!({
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
                    clip("Opening", 0.0, 100.0, 24.0),
                    clip("Interview_A", 100.0, 200.0, 24.0),
                    clip("B_Roll", 300.0, 50.0, 24.0)
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "A1",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    clip("Ambient", 0.0, 350.0, 48.0),
                    clip("Music_Cue", 50.0, 120.0, 48.0),
                    clip("Dialogue_A", 100.0, 180.0, 48.0)
                ]
            }
        ]
    })
    .to_string()
}

pub fn complex() -> String {
    json!({
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "FeatureFilm_RoughCut",
        "metadata": {"project": "feature", "version": 3},
        "global_start_time": {"value": 0.0, "rate": 24.0},
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V1_Main",
                "kind": "Video",
                "metadata": {"resolution": "4K"},
                "children": [
                    clip("Scene1_Take3", 0.0, 500.0, 24.0),
                    transition("Dissolve_1", 12.0, 12.0, 24.0),
                    clip("Scene2_Take1", 500.0, 600.0, 24.0),
                    clip("Scene2_Take2", 1100.0, 400.0, 24.0),
                    transition("Dissolve_2", 10.0, 10.0, 24.0),
                    clip("Scene3_Take1", 1500.0, 800.0, 24.0)
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "V2_BRoll",
                "kind": "Video",
                "metadata": {},
                "children": [
                    clip("Aerial_City", 100.0, 200.0, 24.0),
                    clip("CloseUp_Hands", 400.0, 100.0, 24.0),
                    clip("WideShot_Office", 800.0, 150.0, 24.0),
                    clip("TimeLapse", 1200.0, 300.0, 24.0),
                    clip("Product_Closeup", 1800.0, 200.0, 24.0)
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "A1_Dialogue",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    clip("Boom_Scene1", 0.0, 480.0, 48.0),
                    clip("Lav_Scene2_A", 500.0, 300.0, 48.0),
                    clip("Lav_Scene2_B", 800.0, 300.0, 48.0),
                    clip("Boom_Scene3", 1500.0, 780.0, 48.0)
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "A2_Music",
                "kind": "Audio",
                "metadata": {},
                "children": [
                    clip("Theme_Soft", 0.0, 1500.0, 48.0),
                    clip("Theme_Build", 1500.0, 800.0, 48.0)
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "S1_Subtitles",
                "kind": "Video",
                "metadata": {"format": "SRT"},
                "children": [
                    clip("Sub_Scene1", 0.0, 500.0, 24.0),
                    clip("Sub_Scene2", 500.0, 1000.0, 24.0),
                    clip("Sub_Scene3", 1500.0, 800.0, 24.0)
                ]
            }
        ]
    })
    .to_string()
}

pub fn nested() -> String {
    json!({
        "OTIO_SCHEMA": "otio.schema.Timeline",
        "name": "NestedSequence",
        "metadata": {
            "project": "documentary",
            "color_space": "Rec709",
            "frame_rate": 24.0
        },
        "tracks": [
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Main_Sequence",
                "kind": "Video",
                "metadata": {"vfx_status": "wip"},
                "children": [
                    clip("Act1_Intro", 0.0, 1200.0, 24.0),
                    {
                        "OTIO_SCHEMA": "otio.schema.Track",
                        "name": "Act1_Nested",
                        "kind": "Video",
                        "metadata": {"nested": true},
                        "children": [
                            clip("Interview_SubjectA", 100.0, 400.0, 24.0),
                            clip("Interview_SubjectB", 500.0, 350.0, 24.0),
                            clip("Archival_Footage", 200.0, 200.0, 24.0)
                        ]
                    },
                    clip("Act1_Outro", 1200.0, 300.0, 24.0),
                    clip("Act2_Intro", 1500.0, 200.0, 24.0)
                ]
            },
            {
                "OTIO_SCHEMA": "otio.schema.Track",
                "name": "Audio_Mix",
                "kind": "Audio",
                "metadata": {"mix_level": "-6dB"},
                "children": [
                    clip("Dialogue_Final", 0.0, 2000.0, 48.0),
                    clip("SFX_Ambient", 0.0, 1700.0, 48.0),
                    clip("SFX_Foley", 500.0, 300.0, 48.0),
                    clip("Music_Score", 100.0, 1600.0, 48.0)
                ]
            }
        ]
    })
    .to_string()
}

fn clip(name: &str, start: f64, duration: f64, rate: f64) -> Value {
    json!({
        "OTIO_SCHEMA": "otio.schema.Clip",
        "name": name,
        "metadata": {},
        "source_range": {
            "start_time": {"value": start, "rate": rate},
            "duration": {"value": duration, "rate": rate}
        }
    })
}

fn transition(name: &str, in_offset: f64, out_offset: f64, rate: f64) -> Value {
    json!({
        "OTIO_SCHEMA": "otio.schema.Transition",
        "name": name,
        "metadata": {},
        "in_offset": {"value": in_offset, "rate": rate},
        "out_offset": {"value": out_offset, "rate": rate}
    })
}
