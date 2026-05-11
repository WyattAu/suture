use std::path::PathBuf;

use crate::cmd::user_error;

pub enum HubAction {
    Backup {
        db_path: PathBuf,
        output_dir: PathBuf,
    },
    Restore {
        db_path: PathBuf,
        backup_dir: PathBuf,
    },
}

pub async fn cmd_hub(action: &HubAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        HubAction::Backup {
            db_path,
            output_dir,
        } => {
            let db_path = db_path.clone();
            let output_dir = output_dir.clone();
            let result = tokio::task::spawn_blocking(move || {
                let storage = suture_hub::HubStorage::open(&db_path)?;
                suture_hub::backup::create_backup(&storage, &output_dir)
            })
            .await?;

            match result {
                Ok(manifest) => {
                    println!("Backup created successfully:");
                    println!("  Version:  {}", manifest.version);
                    println!("  Timestamp: {}", manifest.timestamp);
                    println!("  Repos:     {}", manifest.repo_count);
                    println!("  Patches:   {}", manifest.patch_count);
                    println!("  Blobs:     {}", manifest.blob_count);
                    Ok(())
                }
                Err(e) => Err(user_error("backup failed", e)),
            }
        }
        HubAction::Restore {
            db_path,
            backup_dir,
        } => {
            let db_path = db_path.clone();
            let backup_dir = backup_dir.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut storage = suture_hub::HubStorage::open(&db_path)?;
                suture_hub::backup::restore_backup(&mut storage, &backup_dir)
            })
            .await?;

            match result {
                Ok(stats) => {
                    println!("Restore completed successfully:");
                    println!("  Repos restored:   {}", stats.repos_restored);
                    println!("  Patches restored: {}", stats.patches_restored);
                    println!("  Blobs restored:   {}", stats.blobs_restored);
                    Ok(())
                }
                Err(e) => Err(user_error("restore failed", e)),
            }
        }
    }
}
