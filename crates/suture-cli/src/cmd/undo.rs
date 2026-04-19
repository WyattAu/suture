use suture_core::repository::ResetMode;

/// Undo the last operation(s) by walking the reflog.
///
/// Unlike `reset HEAD~N` which only walks commit parents, this reads the
/// reflog to find the previous HEAD position — so it can undo merges,
/// checkouts, cherry-picks, and any other HEAD-moving operation.
pub(crate) async fn cmd_undo(
    steps: Option<usize>,
    hard: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let n = steps.unwrap_or(1);

    // Get the old_head from the Nth most recent reflog entry
    let target_id = repo
        .reflog_older_head(n)?
        .ok_or("no reflog entry to undo (repository may be empty or new)")?;

    let mode = if hard {
        ResetMode::Hard
    } else {
        ResetMode::Soft
    };

    let target_hex = target_id.to_hex();
    let result_id = repo.reset(&target_hex, mode)?;

    // Show what was undone
    let entries = repo.reflog_entries()?;
    let label = if n == 1 { "operation" } else { "operations" };

    // Show the entries we jumped over
    if !hard {
        println!("Undid {} {}: HEAD is now at {}", n, label, &result_id.to_hex()[..8]);
    } else {
        println!(
            "Undid {} {} (hard): HEAD is now at {}",
            n,
            label,
            &result_id.to_hex()[..8]
        );
    }

    // Show what the reflog entry was
    if let Some(entry) = entries.get(n.saturating_sub(1)) {
        println!("  ({})", entry.message);
    }

    Ok(())
}
