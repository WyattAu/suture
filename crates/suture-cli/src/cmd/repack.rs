pub async fn cmd_repack(
    threshold: usize,
    dry_run: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let cas = repo.cas();

    let loose_count = cas.blob_count()?;
    let loose_size = cas.total_size()?;

    let packed_hashes = cas.list_blobs_packed().unwrap_or_default();
    let pack_count = packed_hashes.len();

    let existing_packs = std::fs::read_dir(cas.pack_dir())
        .ok()
        .map_or(0, |entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".pack")))
                .count()
        });

    println!("Repack statistics:");
    println!("  Loose objects:   {loose_count}");
    println!("  Loose size:      {loose_size} bytes");
    println!("  Packed objects:  {pack_count}");
    println!("  Pack files:      {existing_packs}");
    println!("  Threshold:       {threshold} loose objects");

    if loose_count <= threshold as u64 && !force {
        println!(
            "\nNothing to pack ({loose_count} loose objects <= threshold of {threshold})."
        );
        println!("Use --force to pack regardless of threshold.");
        return Ok(());
    }

    if dry_run {
        println!("\nDry run: would pack {loose_count} loose objects.");
        return Ok(());
    }

    let packed = cas.repack(threshold)?;
    if packed == 0 {
        println!("\nNo objects were packed.");
        return Ok(());
    }

    let new_pack_count = std::fs::read_dir(cas.pack_dir())
        .ok()
        .map_or(0, |entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".pack")))
                .count()
        });

    let new_loose_count = cas.blob_count()?;
    let new_packed_hashes = cas.list_blobs_packed().unwrap_or_default();

    let space_saved = if loose_size > 0 && loose_count > 0 {
        let avg_size = loose_size / loose_count;
        avg_size * packed as u64
    } else {
        0
    };

    println!(
        "\nPacked {} objects into {} pack file(s).",
        packed,
        new_pack_count - existing_packs
    );
    println!("  Loose objects remaining:  {new_loose_count}");
    println!("  Total packed objects:     {}", new_packed_hashes.len());
    println!("  Estimated space freed:    {space_saved} bytes");
    println!("  Pack files total:         {new_pack_count}");

    Ok(())
}
