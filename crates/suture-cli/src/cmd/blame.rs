pub(crate) async fn cmd_blame(
    path: &str,
    at: Option<&str>,
    lines: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.blame(path, at)?;

    let (start, end) = if let Some(range) = lines {
        let parts: Vec<&str> = range.split(',').collect();
        if parts.len() != 2 {
            let msg = format!(
                "invalid line range format: '{}' (expected start,end)",
                range
            );
            eprintln!("error: {msg}");
            std::process::exit(1);
        }
        let s: usize = parts[0].parse().map_err(|_| {
            let msg = format!("invalid start line: '{}'", parts[0]);
            eprintln!("error: {msg}");
            std::process::exit(1);
        })?;
        let e: usize = parts[1].parse().map_err(|_| {
            let msg = format!("invalid end line: '{}'", parts[1]);
            eprintln!("error: {msg}");
            std::process::exit(1);
        })?;
        if s == 0 || e == 0 || s > e {
            let msg = format!("invalid line range: {} > {} or zero values", s, e);
            eprintln!("error: {msg}");
            std::process::exit(1);
        }
        (s, e)
    } else {
        (1, usize::MAX)
    };

    for entry in &entries {
        if entry.line_number < start || entry.line_number > end {
            continue;
        }
        let short_hash = entry.patch_id.to_hex().chars().take(8).collect::<String>();
        if entry.patch_id == suture_common::Hash::ZERO {
            println!("{:>4} | {}", entry.line_number, entry.line);
        } else {
            println!(
                "{:>4} | {} ({}) {}",
                entry.line_number, short_hash, entry.author, entry.line
            );
        }
    }
    Ok(())
}
