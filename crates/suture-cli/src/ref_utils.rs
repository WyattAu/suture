pub(crate) fn resolve_ref<'a>(
    repo: &suture_core::repository::Repository,
    ref_str: &str,
    all_patches: &'a [suture_core::patch::types::Patch],
) -> Result<&'a suture_core::patch::types::Patch, Box<dyn std::error::Error>> {
    if ref_str == "HEAD" || ref_str.starts_with("HEAD~") {
        let (_branch_name, head_id) = repo.head().map_err(|e| e.to_string())?;
        let mut target_id = head_id;
        if let Some(n_str) = ref_str.strip_prefix("HEAD~") {
            let n: usize = n_str
                .parse()
                .map_err(|_| format!("invalid HEAD~N: {}", n_str))?;
            for _ in 0..n {
                let patch = all_patches
                    .iter()
                    .find(|p| p.id == target_id)
                    .ok_or_else(|| String::from("HEAD ancestor not found in patches"))?;
                target_id = *patch
                    .parent_ids
                    .first()
                    .ok_or_else(|| String::from("HEAD has no parent"))?;
            }
        }
        return all_patches
            .iter()
            .find(|p| p.id == target_id)
            .ok_or_else(|| "HEAD not found in patches".into());
    }

    {
        let branches = repo.list_branches();
        for (name, target_id) in &branches {
            if name == ref_str {
                return all_patches
                    .iter()
                    .find(|p| p.id == *target_id)
                    .ok_or_else(|| "branch tip not found in patches".into());
            }
        }
    }
    if let Ok(Some(target_id)) = repo.resolve_tag(ref_str) {
        return all_patches
            .iter()
            .find(|p| p.id == target_id)
            .ok_or_else(|| "tag target not found in patches".into());
    }
    let matches: Vec<&suture_core::patch::types::Patch> = all_patches
        .iter()
        .filter(|p| p.id.to_hex().starts_with(ref_str))
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => {
            let mut candidates: Vec<String> = Vec::new();
            for (name, _) in &repo.list_branches() {
                candidates.push(name.clone());
            }
            if let Ok(tags) = repo.list_tags() {
                for (name, _) in &tags {
                    candidates.push(name.clone());
                }
            }
            if let Some(suggestion) = crate::fuzzy::suggest(ref_str, &candidates) {
                Err(format!(
                    "unknown ref: '{}' (did you mean '{}'?)",
                    ref_str, suggestion
                )
                .into())
            } else {
                Err(format!("unknown ref: {}", ref_str).into())
            }
        }
        n => Err(format!("ambiguous ref '{}' matches {} commits", ref_str, n).into()),
    }
}

pub(crate) fn parse_time_filter(s: &str) -> Result<u64, String> {
    if let Some(rest) = s.strip_suffix(" ago") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() == 2
            && let Ok(n) = parts[0].parse::<u64>()
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let seconds = match parts[1] {
                "second" | "seconds" => n,
                "minute" | "minutes" => n * 60,
                "hour" | "hours" => n * 3600,
                "day" | "days" => n * 86400,
                "week" | "weeks" => n * 86400 * 7,
                "month" | "months" => n * 86400 * 30,
                "year" | "years" => n * 86400 * 365,
                _ => return Err(format!("unknown time unit: {}", parts[1])),
            };
            return Ok(now.saturating_sub(seconds));
        }
    }

    let date_str = s.trim();
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() == 3
        && let (Ok(year), Ok(month), Ok(day)) = (
            parts[0].parse::<u64>(),
            parts[1].parse::<u64>(),
            parts[2].parse::<u64>(),
        )
        && (1970..=2100).contains(&year)
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
    {
        let mut ts: u64 = 0;
        let mut y = 1970;
        while y < year {
            let days_in_year =
                if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) {
                    366
                } else {
                    365
                };
            ts += days_in_year * 86400;
            y += 1;
        }
        for m in 1..month {
            ts += days_in_month(year, m) * 86400;
        }
        ts += (day - 1) * 86400;
        return Ok(ts);
    }

    Err(format!("invalid time filter: {}", s))
}

fn days_in_month(_year: u64, month: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let year = _year;
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}
