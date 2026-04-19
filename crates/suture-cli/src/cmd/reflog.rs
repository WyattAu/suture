pub(crate) async fn cmd_reflog() -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let entries = repo.reflog_entries()?;
    if entries.is_empty() {
        println!("No reflog entries.");
        return Ok(());
    }
    for entry in &entries {
        let short_new = &entry.new_head.to_hex()[..8];
        let short_old = &entry.old_head.to_hex()[..8];
        let time_str = format_timestamp(entry.timestamp);
        println!(
            "{} {} {}  {}",
            short_new, short_old, time_str, entry.message
        );
    }
    Ok(())
}

/// Format a unix timestamp as a human-readable relative or absolute time.
fn format_timestamp(unix_ts: i64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let entry_time = UNIX_EPOCH
        .checked_add(Duration::from_secs(unix_ts.unsigned_abs()))
        .unwrap_or(UNIX_EPOCH);
    let now = SystemTime::now();

    match now.duration_since(entry_time) {
        Ok(delta) if delta.as_secs() < 60 => "just now".to_string(),
        Ok(delta) if delta.as_secs() < 3600 => {
            format!("{} min ago", delta.as_secs() / 60)
        }
        Ok(delta) if delta.as_secs() < 86400 => {
            format!("{} hours ago", delta.as_secs() / 3600)
        }
        Ok(delta) => {
            format!("{} days ago", delta.as_secs() / 86400)
        }
        Err(_) => unix_ts.to_string(),
    }
}
