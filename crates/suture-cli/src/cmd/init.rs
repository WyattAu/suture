use std::path::{Path as StdPath, PathBuf};

use crate::cmd::user_error;
use crate::style::{ANSI_BOLD_CYAN, ANSI_RESET};

struct TemplateEntry {
    path: &'static str,
    content: Option<String>,
}

fn get_template(name: &str) -> Result<Vec<TemplateEntry>, String> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    match name {
        "document" => Ok(vec![
            TemplateEntry { path: ".sutureignore", content: Some("# Temporary files\n*.tmp\n~$*\n.DS_Store\nThumbs.db\n".into()) },
            TemplateEntry { path: "README.md", content: Some(format!("# Project Repository\nInitialized by Suture on {date}\n")) },
            TemplateEntry { path: "templates", content: None },
        ]),
        "video" => Ok(vec![
            TemplateEntry { path: ".sutureignore", content: Some("# Temporary files\n*.tmp\n.DS_Store\n# Python cache\n__pycache__/\n*.pyc\n# Render output\nrender_cache/\n# Proxy media (large intermediate files)\nmedia/proxy/\n# Render cache\n.cache/\n*.render\n".into()) },
            TemplateEntry { path: "README.md", content: Some(format!(
                "# Video Project Repository\nInitialized by Suture on {date}\n\n\
                 ## Structure\n\
                 - timelines/ — OTIO timeline files (.otio)\n\
                 - media/      — Source media files\n\
                 - edits/      — Edit decision lists and notes\n\
                 \n\
                 ## Workflow\n\
                 1. Import timelines: `suture timeline import my_timeline.otio`\n\
                 2. View summary:     `suture timeline summary`\n\
                 3. Diff versions:    `suture timeline diff --detailed`\n\
                 4. Export for NLE:   `suture timeline export output.otio`\n"
            )) },
            TemplateEntry { path: "timelines", content: None },
            TemplateEntry { path: "media", content: None },
            TemplateEntry { path: "edits", content: None },
            TemplateEntry { path: "footage", content: None },
            TemplateEntry { path: "exports", content: None },
        ]),
        "data" => Ok(vec![
            TemplateEntry { path: ".sutureignore", content: Some("# Temporary files\n*.tmp\n.DS_Store\n# Build directories\nnode_modules/\ntarget/\ndist/\nbuild/\n".into()) },
            TemplateEntry { path: "README.md", content: Some(format!("# Data Project Repository\nInitialized by Suture on {date}\n")) },
        ]),
        "report" => Ok(vec![
            TemplateEntry { path: ".sutureignore", content: Some("# Temporary files\n*.tmp\n~$*\n.DS_Store\nThumbs.db\n# Archive\narchive/\n".into()) },
            TemplateEntry { path: "README.md", content: Some(format!(
                "# Report Repository\nInitialized by Suture on {date}\n\n\
                 ## Structure\n\
                 - drafts/ — Work in progress\n\
                 - review/ — Ready for review\n\
                 - published/ — Final versions\n\
                 - archive/ — Past reports\n"
            )) },
            TemplateEntry { path: "drafts", content: None },
            TemplateEntry { path: "review", content: None },
            TemplateEntry { path: "published", content: None },
            TemplateEntry { path: "archive", content: None },
        ]),
        _ => Err(format!(
            "unknown template: {name} (expected: video, document, data, report)"
        )),
    }
}

fn apply_template(
    repo_path: &StdPath,
    template_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let entries = get_template(template_name)?;
    let mut created_files = Vec::new();

    for entry in &entries {
        let full_path = repo_path.join(entry.path);
        if let Some(content) = &entry.content {
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    user_error(
                        &format!("failed to create directory '{}'", parent.display()),
                        e,
                    )
                })?;
            }
            std::fs::write(&full_path, content)
                .map_err(|e| user_error(&format!("failed to write '{}'", entry.path), e))?;
            created_files.push(entry.path.to_string());
        } else {
            std::fs::create_dir_all(&full_path).map_err(|e| {
                user_error(&format!("failed to create directory '{}'", entry.path), e)
            })?;
            created_files.push(format!("{}/", entry.path));
        }
    }

    println!("Applied template '{template_name}':");
    for f in &created_files {
        println!("  created {f}");
    }
    Ok(())
}

pub(crate) async fn cmd_init(
    path: &str,
    repo_type: Option<&str>,
    template: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = PathBuf::from(path);

    if repo_path.join(".suture").exists() {
        return Err("already a suture repository (use 'suture doctor' to check health)".into());
    }

    let repo = suture_core::repository::Repository::init(&repo_path, "unknown")
        .map_err(|e| user_error("failed to initialize repository", e))?;

    let resolved_type = if let Some(ty) = repo_type {
        suture_core::file_type::RepoType::from_str_value(ty)
            .ok_or_else(|| format!("unknown repo type '{ty}' (expected: video, document, data)"))?
    } else {
        let detected = suture_core::file_type::auto_detect_repo_type(&repo_path);
        if let Some(rt) = detected {
            println!("Auto-detected repo type: {}", rt.as_str());
            rt
        } else {
            println!("No specific repo type detected (generic repository)");
            drop(repo);
            println!(
                "Initialized empty Suture repository in {}",
                repo_path.display()
            );
            println!("Hint: run `suture config user.name \"Your Name\"` to set your identity");
            show_onboarding_if_first_run();
            return Ok(());
        }
    };

    let config_dir = repo_path.join(".suture");
    let config_path = config_dir.join("config");
    let config_entry = format!("repo.type = \"{}\"", resolved_type.as_str());

    if config_path.exists() {
        let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
        if !existing.contains("repo.type") {
            let updated = format!("{existing}\n{config_entry}\n");
            std::fs::write(&config_path, updated)
                .map_err(|e| user_error("failed to write config", e))?;
        }
    } else {
        std::fs::write(&config_path, format!("{config_entry}\n"))
            .map_err(|e| user_error("failed to write config", e))?;
    }

    let effective_template = template.or(repo_type);
    if let Some(tmpl) = effective_template {
        apply_template(&repo_path, tmpl).map_err(|e| user_error("failed to apply template", e))?;
    }

    println!(
        "Initialized {} Suture repository in {}",
        resolved_type.as_str(),
        repo_path.display()
    );
    println!("Hint: run `suture config user.name \"Your Name\"` to set your identity");

    drop(repo);
    show_onboarding_if_first_run();
    Ok(())
}

fn show_onboarding_if_first_run() {
    let global_config = suture_core::metadata::global_config::GlobalConfig::config_path();

    if global_config.exists() {
        return;
    }

    let version = env!("CARGO_PKG_VERSION");
    let banner = format!(
        "\
{bold}╭─────────────────────────────────────────────╮{reset}
{bold}│  Welcome to Suture v{version}!{pad}│{reset}
{bold}│  Universal Semantic Version Control{pad}│{reset}
{bold}╰─────────────────────────────────────────────╯{reset}

Suture tracks changes to ALL your files — not just code.
It understands Word, Excel, PowerPoint, JSON, and 16+ formats.

Quick start:
  suture add .              Stage all files
  suture commit \"initial\"   Save a snapshot
  suture branch feature     Create a branch
  suture merge feature      Merge changes back

Learn more:
  suture help               Show all commands
  suture docs               Open documentation

Your name and email will be used in commits.
Configure them with:
  suture config user.name \"Your Name\"
  suture config user.email \"you@example.com\"",
        bold = ANSI_BOLD_CYAN,
        reset = ANSI_RESET,
        pad = " ",
        version = version,
    );

    eprintln!();
    eprintln!("{banner}");
    eprintln!();
}
