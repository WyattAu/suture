pub async fn cmd_rev_parse(
    refs: &[String],
    short: bool,
    verify: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;

    let input_refs = if refs.is_empty() {
        &["HEAD".to_owned()]
    } else {
        refs
    };

    for r#ref in input_refs {
        if let Ok(hash) = repo.resolve_ref(r#ref) {
            if short {
                let short_hash = hash.to_hex().chars().take(8).collect::<String>();
                println!("{short_hash}");
            } else {
                println!("{}", hash.to_hex());
            }
        } else {
            if verify {
                let msg = format!("error: '{ref}' is not a valid ref");
                eprintln!("{msg}");
                std::process::exit(1);
            }
            let msg = format!("error: unknown ref '{ref}'");
            eprintln!("{msg}");
        }
    }

    Ok(())
}
