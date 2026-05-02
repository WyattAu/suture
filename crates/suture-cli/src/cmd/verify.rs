use crate::ref_utils::resolve_ref;

pub async fn cmd_verify(
    commit_ref: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    let patches = repo.all_patches();
    let patch = resolve_ref(&repo, commit_ref, &patches)?.clone();

    let pub_key = repo.meta().get_public_key(&patch.author)?;
    let sig = repo.meta().get_signature(&patch.id.to_hex())?;

    if pub_key.is_none() || sig.is_none() {
        println!("NOT SIGNED");
        return Ok(());
    }

    let pub_key_bytes: [u8; 32] = pub_key
        .unwrap()
        .try_into()
        .map_err(|_| "invalid public key length")?;
    let sig_bytes: [u8; 64] = sig
        .unwrap()
        .try_into()
        .map_err(|_| "invalid signature length")?;

    let canonical = suture_core::signing::canonical_patch_bytes(
        &patch.operation_type.to_string(),
        &patch.touch_set.addresses(),
        &patch.target_path,
        &patch.payload,
        &patch.parent_ids,
        &patch.author,
        &patch.message,
        patch.timestamp,
    );

    match suture_core::signing::verify_signature(&pub_key_bytes, &canonical, &sig_bytes) {
        Ok(()) => {
            if verbose {
                let fingerprint = hex::encode(pub_key_bytes);
                println!("VALID");
                println!("  Author:  {}", patch.author);
                println!("  Key:     {fingerprint}");
                println!("  Patch:   {}", patch.id.to_hex());
            } else {
                println!("VALID");
            }
        }
        Err(e) => {
            println!("INVALID: {e}");
        }
    }

    Ok(())
}
