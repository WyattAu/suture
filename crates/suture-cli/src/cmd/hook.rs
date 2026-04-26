use std::path::Path;

const KNOWN_HOOKS: &[&str] = &[
    "pre-commit",
    "post-commit",
    "pre-push",
    "post-push",
    "pre-merge",
    "post-merge",
    "pre-rebase",
    "post-rebase",
    "pre-cherry-pick",
];

pub(crate) enum HookAction {
    List,
    Run { name: String },
    Edit { name: String },
}

pub(crate) async fn cmd_hook(action: &HookAction) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let hooks_dir = suture_core::hooks::hooks_dir(repo.root());

    match action {
        HookAction::List => cmd_hook_list(&hooks_dir),
        HookAction::Run { name } => cmd_hook_run(repo.root(), name).await,
        HookAction::Edit { name } => cmd_hook_edit(&hooks_dir, name),
    }
}

fn cmd_hook_list(hooks_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut any_found = false;

    for hook_name in KNOWN_HOOKS {
        let hook_path = hooks_dir.join(hook_name);
        if hook_path.exists() {
            any_found = true;
            let meta = hook_path.metadata()?;
            let size = meta.len();
            let executable = is_executable(&hook_path);
            let status = if executable {
                "active"
            } else {
                "inactive (not executable)"
            };

            let size_str = if size < 1024 {
                format!("{} B", size)
            } else if size < 1024 * 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            };

            println!("{:<20}  {:>8}  {}", hook_name, size_str, status);

            let sub_dir = hooks_dir.join(format!("{}.d", hook_name));
            if sub_dir.is_dir() {
                let mut entries: Vec<_> = std::fs::read_dir(&sub_dir)?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_file())
                    .collect();
                entries.sort_by_key(|e| e.file_name());
                for entry in &entries {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let sub_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let sub_exec = is_executable(&entry.path());
                    let sub_status = if sub_exec { "active" } else { "inactive" };
                    println!("  {}/{}  {:>8}  {}", hook_name, name, sub_size, sub_status);
                }
            }
        }
    }

    if !any_found {
        println!("No hooks configured.");
        println!("Create hooks with: suture hook edit <name>");
        println!();
        println!("Available hooks:");
        for name in KNOWN_HOOKS {
            println!("  {}", name);
        }
    }

    Ok(())
}

async fn cmd_hook_run(repo_root: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let env = suture_core::hooks::build_env(
        repo_root,
        name,
        None,
        None,
        None,
        std::collections::HashMap::new(),
    );

    let hook_path = suture_core::hooks::hooks_dir(repo_root).join(name);
    if !hook_path.exists() {
        return Err(format!("hook '{}' not found", name).into());
    }

    if !is_executable(&hook_path) {
        return Err(format!("hook '{}' exists but is not executable", name).into());
    }

    let start = std::time::Instant::now();

    match suture_core::hooks::run_hooks(repo_root, name, &env) {
        Ok(results) => {
            let elapsed = start.elapsed();
            for result in &results {
                if !result.stdout.is_empty() {
                    print!("{}", result.stdout);
                }
                if !result.stderr.is_empty() {
                    eprint!("{}", result.stderr);
                }
            }
            println!(
                "{} hook(s) completed in {:.1}s",
                results.len(),
                elapsed.as_secs_f64()
            );
            Ok(())
        }
        Err(suture_core::hooks::HookError::NotFound(_)) => {
            Err(format!("hook '{}' not found", name).into())
        }
        Err(e) => Err(format!("{}: {}", name, e).into()),
    }
}

fn cmd_hook_edit(hooks_dir: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let hook_path = hooks_dir.join(name);

    if !hooks_dir.exists() {
        std::fs::create_dir_all(hooks_dir)?;
    }

    if !hook_path.exists() {
        let default_content = format!(
            "#!/bin/sh\n# Suture {} hook\n# Runs automatically before {}\n\nexit 0\n",
            name,
            name.strip_prefix("pre-")
                .unwrap_or(name.strip_prefix("post-").unwrap_or(name))
        );
        std::fs::write(&hook_path, default_content)?;
        make_executable(&hook_path);
        println!("Created new hook: {}", hook_path.display());
    }

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    let status = std::process::Command::new(&editor)
        .arg(&hook_path)
        .status()
        .map_err(|e| format!("failed to run editor '{}': {}", editor, e))?;

    if !status.success() {
        return Err(format!("editor exited with code {:?}", status.code()).into());
    }

    make_executable(&hook_path);
    println!("Hook saved: {}", hook_path.display());

    Ok(())
}

#[allow(clippy::needless_return)]
fn is_executable(path: &Path) -> bool {
    let is_file = path.is_file();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return is_file
            && path
                .metadata()
                .map(|m| (m.permissions().mode() & 0o111) != 0)
                .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        return is_file;
    }
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata() {
            let mut perms = meta.permissions();
            perms.set_mode(perms.mode() | 0o755);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}
