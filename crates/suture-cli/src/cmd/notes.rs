use crate::ref_utils::resolve_ref;

pub(crate) async fn cmd_notes(
    action: &crate::NotesAction,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    match action {
        crate::NotesAction::Add {
            commit,
            message,
            append,
        } => {
            let target = resolve_ref(&repo, commit, &patches)?;
            let patch_id = target.id;
            let msg = message.clone().unwrap_or_else(|| {
                eprintln!("Enter note (Ctrl+D to finish):");
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf).unwrap_or_default();
                buf.trim_end().to_string()
            });
            if *append {
                let existing = repo.list_notes(&patch_id)?;
                if let Some(last_note) = existing.last() {
                    let appended = format!("{}\n---\n{}", last_note, msg);
                    repo.remove_note(&patch_id, existing.len() - 1)?;
                    repo.add_note(&patch_id, &appended)?;
                    println!("Note appended to {}", commit);
                } else {
                    repo.add_note(&patch_id, &msg)?;
                    println!("Note added to {}", commit);
                }
            } else {
                repo.add_note(&patch_id, &msg)?;
                println!("Note added to {}", commit);
            }
        }
        crate::NotesAction::List { commit } | crate::NotesAction::Show { commit } => {
            let target = resolve_ref(&repo, commit, &patches)?;
            let patch_id = target.id;
            let notes = repo.list_notes(&patch_id)?;
            if notes.is_empty() {
                println!("No notes for commit {}.", commit);
            } else {
                for (i, note) in notes.iter().enumerate() {
                    println!("Note {}: {}", i, note);
                }
            }
        }
        crate::NotesAction::Remove { commit, index } => {
            let target = resolve_ref(&repo, commit, &patches)?;
            let patch_id = target.id;
            repo.remove_note(&patch_id, *index)?;
            println!("Removed note {} from {}", index, commit);
        }
    }
    Ok(())
}
