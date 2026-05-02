pub async fn cmd_audit(
    verify: bool,
    show: bool,
    count: bool,
    tail: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    use suture_core::audit::AuditLog;

    let root = Path::new(".");
    let audit_path = root.join(".suture").join("audit").join("chain.log");
    let audit = AuditLog::open(&audit_path)?;

    if verify {
        let (total, first_invalid) = audit.verify_chain()?;
        match first_invalid {
            None => println!("VALID: {total} entries"),
            Some(i) => println!("TAMPERED: first invalid at entry {i}"),
        }
        return Ok(());
    }

    if count {
        let entries = audit.entries()?;
        println!("{}", entries.len());
        return Ok(());
    }

    let entries = audit.entries()?;
    let display: Vec<_> = if show {
        entries
    } else {
        let n = tail.unwrap_or(10).min(entries.len());
        entries.into_iter().rev().take(n).rev().collect()
    };

    if display.is_empty() {
        println!("No audit entries.");
        return Ok(());
    }

    println!(
        "{:<6} {:<30} {:<15} {:<10} DETAILS",
        "SEQ", "TIMESTAMP", "ACTOR", "ACTION"
    );
    println!("{}", "-".repeat(100));
    for entry in &display {
        println!(
            "{:<6} {:<30} {:<15} {:<10} {}",
            entry.sequence,
            &entry.timestamp[..entry.timestamp.len().min(29)],
            &entry.actor[..entry.actor.len().min(14)],
            &entry.action[..entry.action.len().min(9)],
            &entry.details[..entry.details.len().min(40)],
        );
    }

    Ok(())
}
