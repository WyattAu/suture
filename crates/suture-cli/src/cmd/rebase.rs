use crate::cmd::user_error;
use crate::style::run_hook_if_exists;

pub async fn cmd_rebase(
    branch: &str,
    interactive: bool,
    resume: bool,
    abort: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;

    let branches: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
    if !branches.contains(&branch.to_owned()) && repo.resolve_ref(branch).is_err() {
        let hint = crate::fuzzy::suggest(branch, &branches)
            .map_or_else(String::new, |suggestion| {
                format!(" (did you mean '{suggestion}'?)")
            });
        return Err(format!(
            "branch '{branch}' not found{hint} (use 'suture branch' to create it)"
        )
        .into());
    }

    let status = repo
        .status()
        .map_err(|e| user_error("failed to check repository status", e))?;
    if !status.staged_files.is_empty() {
        return Err("cannot rebase with staged changes (commit or stash them first)".into());
    }

    // Handle --abort
    if abort {
        repo.rebase_abort()
            .map_err(|e| user_error("failed to abort rebase", e))?;
        println!("Rebase aborted.");
        return Ok(());
    }

    // Handle --continue (resume)
    if resume {
        // For now, --continue is not needed since our interactive rebase
        // runs atomically. The edit action pauses and lets the user amend
        // then run a normal commit. Future: full --continue support.
        eprintln!(
            "Note: --continue is not yet needed. After editing during rebase, run `suture commit` then `suture rebase --continue`."
        );
        return Ok(());
    }

    // Handle interactive rebase
    if interactive {
        return cmd_rebase_interactive(&mut repo, branch).await;
    }

    // Non-interactive rebase (original behavior)
    // Run pre-rebase hook
    let (current_branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_owned(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_owned(), current_branch);
    pre_extra.insert("SUTURE_HEAD".to_owned(), head_id.to_hex());
    pre_extra.insert("SUTURE_REBASE_ONTO".to_owned(), branch.to_owned());
    run_hook_if_exists(repo.root(), "pre-rebase", pre_extra)?;

    let result = repo
        .rebase(branch)
        .map_err(|e| user_error(&format!("rebase onto '{branch}' failed"), e))?;
    if result.patches_replayed > 0 {
        println!(
            "Rebase onto '{}': {} patch(es) replayed",
            branch, result.patches_replayed
        );
    } else {
        println!("Already up to date.");
    }

    // Run post-rebase hook
    let (branch_after, head_after) = repo.head()?;
    let mut post_extra = std::collections::HashMap::new();
    post_extra.insert("SUTURE_BRANCH".to_owned(), branch_after);
    post_extra.insert("SUTURE_HEAD".to_owned(), head_after.to_hex());
    post_extra.insert("SUTURE_REBASE_ONTO".to_owned(), branch.to_owned());
    run_hook_if_exists(repo.root(), "post-rebase", post_extra)?;

    Ok(())
}

async fn cmd_rebase_interactive(
    repo: &mut suture_core::repository::Repository,
    base_branch: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Resolve the base commit
    let base_bn = suture_common::BranchName::new(base_branch)
        .map_err(|e| format!("invalid branch name '{base_branch}': {e}"))?;
    let base_id = repo.dag().get_branch(&base_bn).ok_or_else(|| {
        let branches: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
        crate::fuzzy::suggest(base_branch, &branches).map_or_else(
            || format!("branch '{base_branch}' not found"),
            |suggestion| format!("branch '{base_branch}' not found (did you mean '{suggestion}'?)"),
        )
    })?;

    // Generate TODO file
    let todo_content = repo
        .generate_rebase_todo(&base_id)
        .map_err(|e| user_error("failed to generate rebase plan", e))?;
    if todo_content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .count()
        == 0
    {
        println!("Nothing to rebase.");
        return Ok(());
    }

    // Write TODO to a temp file and open editor
    let todo_path = std::env::temp_dir().join("suture-rebase-todo");
    std::fs::write(&todo_path, &todo_content)?;

    // Open editor
    // SECURITY: Editor is read from env vars (SUTURE_EDITOR > EDITOR).
    // This matches git's behavior. In untrusted environments, set SUTURE_EDITOR
    // to an explicit path. The editor is executed directly (not via shell).
    let editor = std::env::var("SUTURE_EDITOR")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vim".to_owned());

    let status = std::process::Command::new(&editor)
        .arg(&todo_path)
        .status()
        .map_err(|e| format!("failed to run editor '{editor}': {e}"))?;

    if !status.success() {
        std::fs::remove_file(&todo_path).ok();
        return Err(format!("editor '{editor}' exited with non-zero status").into());
    }

    // Read edited TODO
    let edited = std::fs::read_to_string(&todo_path)?;
    std::fs::remove_file(&todo_path).ok();

    // Check if user removed all entries (abort)
    let has_entries = edited
        .lines()
        .any(|l| !l.trim().is_empty() && !l.trim().starts_with('#'));
    if !has_entries {
        println!("Rebase cancelled (no commits selected).");
        return Ok(());
    }

    // Parse TODO into plan
    let plan = repo
        .parse_rebase_todo(&edited, &base_id)
        .map_err(|e| user_error("failed to parse rebase plan", e))?;

    // Show plan summary
    let pick_count = plan
        .entries
        .iter()
        .filter(|e| e.action == suture_core::repository::RebaseAction::Pick)
        .count();
    let drop_count = plan
        .entries
        .iter()
        .filter(|e| e.action == suture_core::repository::RebaseAction::Drop)
        .count();
    let squash_count = plan
        .entries
        .iter()
        .filter(|e| e.action == suture_core::repository::RebaseAction::Squash)
        .count();
    let reword_count = plan
        .entries
        .iter()
        .filter(|e| e.action == suture_core::repository::RebaseAction::Reword)
        .count();

    println!(
        "Rebase plan: {pick_count} pick, {squash_count} squash, {reword_count} reword, {drop_count} drop"
    );

    // Run pre-rebase hook
    let (current_branch, head_id) = repo
        .head()
        .unwrap_or_else(|_| ("main".to_owned(), suture_common::Hash::ZERO));
    let mut pre_extra = std::collections::HashMap::new();
    pre_extra.insert("SUTURE_BRANCH".to_owned(), current_branch);
    pre_extra.insert("SUTURE_HEAD".to_owned(), head_id.to_hex());
    pre_extra.insert("SUTURE_REBASE_ONTO".to_owned(), base_branch.to_owned());
    run_hook_if_exists(repo.root(), "pre-rebase", pre_extra)?;

    // Execute the plan
    let new_tip = repo
        .rebase_interactive(&plan, &base_id)
        .map_err(|e| user_error("interactive rebase failed", e))?;

    let (branch_after, head_after) = repo.head()?;
    let mut post_extra = std::collections::HashMap::new();
    post_extra.insert("SUTURE_BRANCH".to_owned(), branch_after);
    post_extra.insert("SUTURE_HEAD".to_owned(), head_after.to_hex());
    post_extra.insert("SUTURE_REBASE_ONTO".to_owned(), base_branch.to_owned());
    run_hook_if_exists(repo.root(), "post-rebase", post_extra)?;

    println!("Interactive rebase complete. New tip: {new_tip}");

    Ok(())
}
