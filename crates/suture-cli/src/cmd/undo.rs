pub async fn cmd_undo(
    n: usize,
    hard: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use suture_core::repository::ResetMode;

    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    if hard && !force {
        return Err("use --force to discard working tree changes".into());
    }

    let (_, head_id) = repo.head()?;
    let mut current = head_id;
    let mut commit_count: usize = 0;

    loop {
        let patch = repo
            .dag()
            .get_patch(&current)
            .ok_or_else(|| "HEAD commit not found in DAG".to_owned())?;
        match patch.parent_ids.first() {
            Some(parent_id) => {
                current = *parent_id;
                commit_count += 1;
            }
            None => break,
        }
    }

    if n > commit_count {
        return Err(format!(
            "cannot undo: only {} commit{} in history",
            commit_count,
            if commit_count == 1 { "" } else { "s" }
        )
        .into());
    }

    if repo.has_uncommitted_changes()? && !hard {
        eprintln!("warning: you have uncommitted changes");
    }

    let target_ref = format!("HEAD~{n}");
    let mode = if hard {
        ResetMode::Hard
    } else {
        ResetMode::Soft
    };

    let result_id = repo.reset(&target_ref, mode)?;

    if hard {
        println!(
            "Undid {} commit{} (hard): HEAD is now at {}",
            n,
            if n == 1 { "" } else { "s" },
            &result_id.to_hex()[..8]
        );
    } else {
        println!(
            "Undid {} commit{} (soft): HEAD is now at {}",
            n,
            if n == 1 { "" } else { "s" },
            &result_id.to_hex()[..8]
        );
    }

    Ok(())
}
