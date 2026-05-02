use crate::cmd::user_error;

pub async fn cmd_branch(
    name: Option<&str>,
    target: Option<&str>,
    delete: bool,
    list: bool,
    protect: bool,
    unprotect: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))
        .map_err(|e| user_error("failed to open repository", e))?;

    if protect || unprotect {
        let branch_name = name.ok_or("--protect/--unprotect requires a branch name")?;
        let config_key = format!("branch.{branch_name}.protected");
        if protect {
            repo.set_config(&config_key, "true")?;
        } else {
            let _ = repo.meta().delete_config(&config_key);
        }

        if let Ok(url) = repo.get_remote_url("origin") {
            let repo_id = crate::remote_proto::derive_repo_id(&url, "origin");
            let client = reqwest::Client::new();
            let endpoint = if protect { "protect" } else { "unprotect" };
            let resp = client
                .post(format!(
                    "{}/repos/{}/{}",
                    url.trim_end_matches('/'),
                    repo_id,
                    endpoint
                ))
                .send()
                .await?;
            if resp.status().is_success() {
                println!(
                    "Branch '{}' {} on remote",
                    branch_name,
                    if protect { "protected" } else { "unprotected" }
                );
            } else {
                eprintln!(
                    "Warning: could not {} branch on remote: {}",
                    if protect { "protect" } else { "unprotect" },
                    resp.status()
                );
            }
        }

        println!(
            "Branch '{}' {}",
            branch_name,
            if protect { "protected" } else { "unprotected" }
        );
        return Ok(());
    }

    if list || name.is_none() {
        let branches = repo.list_branches();
        if branches.is_empty() {
            println!("No branches.");
        } else {
            let head = repo.head().ok();
            let head_branch = head.as_ref().map(|(n, _)| n.as_str());
            for (bname, _target) in &branches {
                let marker = if head_branch == Some(bname.as_str()) {
                    "* "
                } else {
                    "  "
                };
                let protected = repo
                    .get_config(&format!("branch.{bname}.protected"))?
                    .is_some_and(|v| v == "true");
                let lock = if protected { " [protected]" } else { "" };
                println!("{marker}{bname}{lock}");
            }
        }
        return Ok(());
    }

    let name =
        name.ok_or_else(|| "branch name required (use --list to show branches)".to_owned())?;
    if delete {
        let branches: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
        if !branches.iter().any(|b| b == name) {
            if let Some(suggestion) = crate::fuzzy::suggest(name, &branches) {
                return Err(
                    format!("branch '{name}' not found (did you mean '{suggestion}'?)").into(),
                );
            }
            return Err(format!("branch '{name}' not found (use 'suture branch --list' to see available branches)").into());
        }
        repo.delete_branch(name)
            .map_err(|e| user_error(&format!("failed to delete branch '{name}'"), e))?;
        println!("Deleted branch '{name}'");
    } else {
        let existing: Vec<String> = repo.list_branches().into_iter().map(|(n, _)| n).collect();
        if existing.iter().any(|b| b == name) {
            return Err(format!(
                "branch '{name}' already exists (use 'suture checkout {name}' to switch to it)"
            )
            .into());
        }
        repo.create_branch(name, target)
            .map_err(|e| user_error(&format!("failed to create branch '{name}'"), e))?;
        println!("Created branch '{name}'");
    }
    Ok(())
}
