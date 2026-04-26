use std::path::Path as StdPath;

use crate::ref_utils::resolve_ref;

pub(crate) async fn cmd_timeline(
    action: &crate::TimelineAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        crate::TimelineAction::Import { file, message } => {
            timeline_import(file, message.as_deref()).await
        }
        crate::TimelineAction::Export { output, at } => {
            timeline_export(output, at.as_deref()).await
        }
        crate::TimelineAction::Summary { at } => timeline_summary(at).await,
        crate::TimelineAction::Diff { from, to, detailed } => {
            timeline_diff(from, to, *detailed).await
        }
        crate::TimelineAction::List { otio_only } => timeline_list(*otio_only).await,
    }
}

async fn timeline_import(
    file: &str,
    message: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let src_path = StdPath::new(file);
    if !src_path.exists() {
        return Err(format!("file not found: {file}").into());
    }

    let content =
        std::fs::read_to_string(src_path).map_err(|e| format!("cannot read {file}: {e}"))?;

    let parsed = parse_otio_minimal(&content);
    let filename = src_path.file_name().ok_or("cannot determine filename")?;
    let mut repo = suture_core::repository::Repository::open(StdPath::new("."))?;

    let dest = StdPath::new(filename);
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src_path, dest)?;

    let filename_str = filename.to_string_lossy().to_string();
    repo.add(&filename_str)?;

    let msg = match message {
        Some(m) => m.to_string(),
        None => {
            let name = parsed.name.as_deref().unwrap_or("unnamed");
            format!(
                "Import timeline: {name} ({} clips, {} tracks)",
                parsed.clip_count, parsed.track_count
            )
        }
    };

    repo.commit(&msg)?;

    println!("Imported {filename_str}");
    println!("  Name:   {}", parsed.name.as_deref().unwrap_or("unknown"));
    println!("  Tracks: {}", parsed.track_count);
    println!("  Clips:  {}", parsed.clip_count);
    if let Some(duration) = &parsed.duration {
        println!("  Duration: {duration}");
    }

    Ok(())
}

async fn timeline_export(output: &str, at: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let ref_str = at.unwrap_or("HEAD");
    let patches = repo.all_patches();
    let patch = resolve_ref(&repo, ref_str, &patches)?;
    let tree = repo.snapshot(&patch.id)?;

    let mut otio_files: Vec<String> = Vec::new();
    for (path, _hash) in tree.iter() {
        if path.ends_with(".otio") {
            otio_files.push(path.clone());
        }
    }

    if otio_files.is_empty() {
        return Err(format!("no .otio files found at {ref_str}").into());
    }

    let src_file = &otio_files[0];
    let blob_hash = tree
        .get(src_file)
        .ok_or_else(|| format!("blob not found for {src_file}"))?;
    let data = repo.cas().get_blob(blob_hash)?;

    let out_path = StdPath::new(output);
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, data)?;

    println!("Exported {src_file} -> {output}");
    if otio_files.len() > 1 {
        println!(
            "Note: {} additional .otio file(s) found: {}",
            otio_files.len() - 1,
            &otio_files[1..].join(", ")
        );
    }

    Ok(())
}

async fn timeline_summary(at: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let patches = repo.all_patches();
    let patch = resolve_ref(&repo, at, &patches)?;
    let tree = repo.snapshot(&patch.id)?;

    let mut otio_files: Vec<String> = Vec::new();
    for (path, _hash) in tree.iter() {
        if path.ends_with(".otio") {
            otio_files.push(path.clone());
        }
    }

    if otio_files.is_empty() {
        return Err(format!("no .otio files found at {at}").into());
    }

    for otio_file in &otio_files {
        let blob_hash = tree
            .get(otio_file)
            .ok_or_else(|| format!("blob not found for {otio_file}"))?;
        let data = repo.cas().get_blob(blob_hash)?;
        let content = String::from_utf8_lossy(&data);
        let parsed = parse_otio_minimal(&content);

        println!("Timeline: {}", parsed.name.as_deref().unwrap_or("unknown"));
        println!(
            "Tracks: {} ({} video, {} audio)",
            parsed.track_count, parsed.video_tracks, parsed.audio_tracks
        );
        println!("Clips: {}", parsed.clip_count);
        if let Some(duration) = &parsed.duration {
            println!("Duration: {duration}");
        }
        if otio_files.len() > 1 {
            println!();
        }
    }

    Ok(())
}

async fn timeline_diff(
    from: &str,
    to: &str,
    detailed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let patches = repo.all_patches();

    let from_patch = resolve_ref(&repo, from, &patches)?;
    let to_patch = resolve_ref(&repo, to, &patches)?;
    let from_tree = repo.snapshot(&from_patch.id)?;
    let to_tree = repo.snapshot(&to_patch.id)?;

    let from_otio: std::collections::HashSet<&String> = from_tree
        .iter()
        .filter(|(p, _)| p.ends_with(".otio"))
        .map(|(p, _)| p)
        .collect();
    let to_otio: std::collections::HashSet<&String> = to_tree
        .iter()
        .filter(|(p, _)| p.ends_with(".otio"))
        .map(|(p, _)| p)
        .collect();

    let added: Vec<_> = to_otio.difference(&from_otio).collect();
    let removed: Vec<_> = from_otio.difference(&to_otio).collect();
    let common: Vec<_> = to_otio.intersection(&from_otio).collect();

    let mut changes_found = false;

    for path in &added {
        println!("added: {path}");
        changes_found = true;
    }
    for path in &removed {
        println!("removed: {path}");
        changes_found = true;
    }

    for path in &common {
        let from_hash = from_tree
            .get(path)
            .ok_or_else(|| format!("hash missing for '{}' in source tree", path))?;
        let to_hash = to_tree
            .get(path)
            .ok_or_else(|| format!("hash missing for '{}' in target tree", path))?;
        if from_hash == to_hash {
            continue;
        }

        changes_found = true;
        println!("modified: {path}");

        let from_data = repo.cas().get_blob(from_hash)?;
        let to_data = repo.cas().get_blob(to_hash)?;

        let from_str = String::from_utf8_lossy(&from_data);
        let to_str = String::from_utf8_lossy(&to_data);
        let from_lines: Vec<&str> = from_str.lines().collect();
        let to_lines: Vec<&str> = to_str.lines().collect();

        let diffs = difflib::unified_diff(&from_lines, &to_lines, path, path, "", "", 3);
        for diff_line in diffs {
            print!("{diff_line}");
        }

        if detailed {
            let from_content = String::from_utf8_lossy(&from_data);
            let to_content = String::from_utf8_lossy(&to_data);
            let from_parsed = parse_otio_minimal(&from_content);
            let to_parsed = parse_otio_minimal(&to_content);

            println!("\n  Clip changes:");
            let from_clips: std::collections::HashSet<_> =
                from_parsed.clip_names.iter().cloned().collect();
            let to_clips: std::collections::HashSet<_> =
                to_parsed.clip_names.iter().cloned().collect();

            for clip in to_clips.difference(&from_clips) {
                println!("    + {clip}");
            }
            for clip in from_clips.difference(&to_clips) {
                println!("    - {clip}");
            }
        }
    }

    if !changes_found {
        println!("No changes to .otio files between {from} and {to}");
    }

    Ok(())
}

async fn timeline_list(otio_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
    let tree = repo.snapshot_head()?;

    let mut files: Vec<&String> = Vec::new();
    for (path, _hash) in tree.iter() {
        if otio_only {
            if path.ends_with(".otio") {
                files.push(path);
            }
        } else {
            files.push(path);
        }
    }

    if files.is_empty() {
        if otio_only {
            println!("No .otio files in working tree");
        } else {
            println!("No files in working tree");
        }
        return Ok(());
    }

    files.sort();
    for file in &files {
        println!("{file}");
    }

    Ok(())
}

struct OtioInfo {
    name: Option<String>,
    track_count: usize,
    video_tracks: usize,
    audio_tracks: usize,
    clip_count: usize,
    clip_names: Vec<String>,
    duration: Option<String>,
}

fn parse_otio_minimal(content: &str) -> OtioInfo {
    let value: serde_json::Value = serde_json::from_str(content).unwrap_or_default();

    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut track_count = 0usize;
    let mut video_tracks = 0usize;
    let mut audio_tracks = 0usize;
    let mut clip_count = 0usize;
    let mut clip_names: Vec<String> = Vec::new();

    if let Some(tracks) = value.get("tracks").and_then(|v| v.as_array()) {
        track_count = tracks.len();
        for track in tracks {
            let kind = track.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "Video" | "Sequence" => video_tracks += 1,
                "Audio" => audio_tracks += 1,
                _ => {}
            }
            count_clips_recursive(track, &mut clip_count, &mut clip_names);
        }
    }

    let duration = value
        .get("global_start_time")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64())
        .map(|v| format!("{v:.2}"))
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|m| m.get("timing"))
                .and_then(|t| t.get("duration"))
                .and_then(|v| v.as_f64())
                .map(|v| format!("{v:.2}"))
        });

    OtioInfo {
        name,
        track_count,
        video_tracks,
        audio_tracks,
        clip_count,
        clip_names,
        duration,
    }
}

fn count_clips_recursive(
    value: &serde_json::Value,
    clip_count: &mut usize,
    clip_names: &mut Vec<String>,
) {
    if let Some(ntype) = value.get("OTIO_SCHEMA").and_then(|v| v.as_str())
        && (ntype.contains("Clip") || ntype.contains("Clip.1"))
    {
        *clip_count += 1;
        if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
            clip_names.push(name.to_string());
        }
    }

    if let Some(children) = value.get("children").and_then(|v| v.as_array()) {
        for child in children {
            count_clips_recursive(child, clip_count, clip_names);
        }
    }
}
