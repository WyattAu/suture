pub fn suggest(input: &str, candidates: &[impl AsRef<str>]) -> Option<String> {
    let input_lower = input.to_lowercase();
    let threshold = std::cmp::max(1, input.len() / 3) as u32;

    let mut best: Option<(u32, &str)> = None;
    for candidate in candidates {
        let c = candidate.as_ref();
        let dist = strsim::levenshtein(&input_lower, &c.to_lowercase()) as u32;
        if dist > 0 && dist <= threshold && best.is_none_or(|(best_dist, _)| dist < best_dist) {
            best = Some((dist, c));
        }
    }
    best.map(|(_, s)| s.to_owned())
}
