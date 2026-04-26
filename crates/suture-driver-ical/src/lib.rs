#![allow(clippy::collapsible_match)]
use std::collections::{BTreeMap, BTreeSet, HashMap};

use suture_driver::{DriverError, SemanticChange, SutureDriver};

type Component = (String, Vec<(String, String)>);

pub struct IcalDriver;

impl IcalDriver {
    pub fn new() -> Self {
        Self
    }

    fn unfold_lines(content: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();
        for raw_line in content.lines() {
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if line.starts_with(' ') || line.starts_with('\t') {
                current.push_str(&line[1..]);
            } else {
                if !current.is_empty() {
                    lines.push(current);
                }
                current = line.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    fn parse_ical(content: &str) -> Result<Vec<Component>, DriverError> {
        let lines = Self::unfold_lines(content);
        let mut components: Vec<Component> = Vec::new();
        let mut component_stack: Vec<Component> = Vec::new();

        for line in &lines {
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("BEGIN:") {
                let comp_type = rest.trim();
                component_stack.push((comp_type.to_string(), Vec::new()));
            } else if let Some(rest) = line.strip_prefix("END:") {
                let end_type = rest.trim();
                if let Some((comp_type, props)) = component_stack.pop()
                    && comp_type == end_type
                {
                    if component_stack.is_empty() {
                        components.push((comp_type, props));
                    } else if let Some(parent) = component_stack.last_mut() {
                        parent
                            .1
                            .push((format!("BEGIN:{comp_type}"), comp_type.clone()));
                        for (k, v) in props {
                            parent.1.push((k, v));
                        }
                        parent
                            .1
                            .push((format!("END:{comp_type}"), comp_type.clone()));
                    }
                }
            } else if let Some(entry) = component_stack.last_mut()
                && let Some((key, value)) = Self::parse_property_line(line)
            {
                entry.1.push((key, value));
            }
        }

        Ok(components)
    }

    fn parse_property_line(line: &str) -> Option<(String, String)> {
        let colon_pos = line.find(':')?;
        let value = &line[colon_pos + 1..];
        let prop_part = &line[..colon_pos];

        let prop_name = if let Some(semi_pos) = prop_part.find(';') {
            &prop_part[..semi_pos]
        } else {
            prop_part
        };

        Some((prop_name.to_string(), value.to_string()))
    }

    fn extract_uid(props: &[(String, String)]) -> Option<String> {
        for (key, value) in props {
            if key == "UID" {
                return Some(value.clone());
            }
        }
        None
    }

    fn components_by_uid(components: &[Component]) -> BTreeMap<String, Vec<(String, String)>> {
        let mut map = BTreeMap::new();
        for (comp_type, props) in components {
            if matches!(
                comp_type.as_str(),
                "VEVENT" | "VTODO" | "VJOURNAL" | "VFREEBUSY"
            ) {
                let uid = Self::extract_uid(props).unwrap_or_default();
                let key = format!("{comp_type}[UID={uid}]");
                map.insert(key, props.clone());
            }
        }
        map
    }

    fn diff_properties(
        comp_type: &str,
        uid: &str,
        old_props: &[(String, String)],
        new_props: &[(String, String)],
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();
        let base_path = format!("/VCALENDAR/{comp_type}[UID={uid}]");

        let old_map: HashMap<&str, &str> = old_props
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let new_map: HashMap<&str, &str> = new_props
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let old_keys: BTreeSet<&str> = old_map.keys().copied().collect();
        let new_keys: BTreeSet<&str> = new_map.keys().copied().collect();

        for key in &old_keys {
            if !new_keys.contains(key) {
                changes.push(SemanticChange::Removed {
                    path: format!("{base_path}/{key}"),
                    old_value: old_map[key].to_string(),
                });
            }
        }

        for key in &new_keys {
            if !old_keys.contains(key) {
                changes.push(SemanticChange::Added {
                    path: format!("{base_path}/{key}"),
                    value: new_map[key].to_string(),
                });
            }
        }

        for key in &old_keys {
            if let Some(new_val) = new_keys.contains(key).then(|| new_map[key]) {
                let old_val = old_map[key];
                if old_val != new_val {
                    changes.push(SemanticChange::Modified {
                        path: format!("{base_path}/{key}"),
                        old_value: old_val.to_string(),
                        new_value: new_val.to_string(),
                    });
                }
            }
        }

        changes
    }

    fn serialize_components(components: &[Component]) -> String {
        let mut output = String::new();
        output.push_str("BEGIN:VCALENDAR\r\n");
        output.push_str("VERSION:2.0\r\n");
        output.push_str("PRODID:-//Suture//ICAL//EN\r\n");
        for (comp_type, props) in components {
            output.push_str(&format!("BEGIN:{comp_type}\r\n"));
            for (key, value) in props {
                output.push_str(&format!("{key}:{value}\r\n"));
            }
            output.push_str(&format!("END:{comp_type}\r\n"));
        }
        output.push_str("END:VCALENDAR\r\n");
        output
    }

    fn extract_inner_components(components: &[Component]) -> Vec<Component> {
        let mut inner = Vec::new();
        for (comp_type, props) in components {
            if *comp_type == "VCALENDAR" {
                let mut i = 0;
                while i < props.len() {
                    if let Some(ct) = props[i].0.strip_prefix("BEGIN:") {
                        let mut inner_props = Vec::new();
                        i += 1;
                        while i < props.len() && !props[i].0.starts_with("END:") {
                            inner_props.push(props[i].clone());
                            i += 1;
                        }
                        inner.push((ct.to_string(), inner_props));
                    }
                    i += 1;
                }
            } else {
                inner.push((comp_type.clone(), props.clone()));
            }
        }
        inner
    }

    fn merge_components(
        base: &[Component],
        ours: &[Component],
        theirs: &[Component],
    ) -> Result<Option<Vec<Component>>, DriverError> {
        let base_by_uid = Self::components_by_uid(base);
        let ours_by_uid = Self::components_by_uid(ours);
        let theirs_by_uid = Self::components_by_uid(theirs);

        let all_uids: BTreeSet<String> = base_by_uid
            .keys()
            .chain(ours_by_uid.keys())
            .chain(theirs_by_uid.keys())
            .cloned()
            .collect();

        let mut merged: Vec<Component> = Vec::new();

        for uid_key in &all_uids {
            let in_base = base_by_uid.contains_key(uid_key);
            let in_ours = ours_by_uid.contains_key(uid_key);
            let in_theirs = theirs_by_uid.contains_key(uid_key);

            match (in_base, in_ours, in_theirs) {
                (true, false, false) => continue,
                (false, true, false) => {
                    let comp_type = uid_key.split('[').next().unwrap_or("VEVENT").to_string();
                    merged.push((comp_type, ours_by_uid[uid_key].clone()));
                }
                (false, false, true) => {
                    let comp_type = uid_key.split('[').next().unwrap_or("VEVENT").to_string();
                    merged.push((comp_type, theirs_by_uid[uid_key].clone()));
                }
                (false, true, true) => {
                    if ours_by_uid[uid_key] == theirs_by_uid[uid_key] {
                        let comp_type = uid_key.split('[').next().unwrap_or("VEVENT").to_string();
                        merged.push((comp_type, ours_by_uid[uid_key].clone()));
                    } else {
                        return Ok(None);
                    }
                }
                (true, true, false) => {
                    let comp_type = uid_key.split('[').next().unwrap_or("VEVENT").to_string();
                    merged.push((comp_type, ours_by_uid[uid_key].clone()));
                }
                (true, false, true) => {
                    let comp_type = uid_key.split('[').next().unwrap_or("VEVENT").to_string();
                    merged.push((comp_type, theirs_by_uid[uid_key].clone()));
                }
                (false, false, false) => {}
                (true, true, true) => {
                    let base_props = &base_by_uid[uid_key];
                    let ours_props = &ours_by_uid[uid_key];
                    let theirs_props = &theirs_by_uid[uid_key];

                    if ours_props == theirs_props {
                        let comp_type = uid_key.split('[').next().unwrap_or("VEVENT").to_string();
                        merged.push((comp_type, ours_props.clone()));
                        continue;
                    }

                    let base_map: HashMap<&str, &str> = base_props
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    let ours_map: HashMap<&str, &str> = ours_props
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    let theirs_map: HashMap<&str, &str> = theirs_props
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let all_keys: BTreeSet<&str> = base_map
                        .keys()
                        .chain(ours_map.keys())
                        .chain(theirs_map.keys())
                        .copied()
                        .collect();

                    let mut merged_props = Vec::new();

                    for key in &all_keys {
                        let bv = base_map.get(key).copied();
                        let ov = ours_map.get(key).copied();
                        let tv = theirs_map.get(key).copied();

                        match (bv, ov, tv) {
                            (_, Some(o), None) => {
                                merged_props.push((key.to_string(), o.to_string()))
                            }
                            (_, None, Some(t)) => {
                                merged_props.push((key.to_string(), t.to_string()))
                            }
                            (_, Some(o), Some(t)) => {
                                if o == t {
                                    merged_props.push((key.to_string(), o.to_string()));
                                } else if o == bv.unwrap_or("") {
                                    merged_props.push((key.to_string(), t.to_string()));
                                } else if t == bv.unwrap_or("") {
                                    merged_props.push((key.to_string(), o.to_string()));
                                } else {
                                    return Ok(None);
                                }
                            }
                            (_, None, None) => {}
                        }
                    }

                    let comp_type = uid_key.split('[').next().unwrap_or("VEVENT").to_string();
                    merged.push((comp_type, merged_props));
                }
            }
        }

        Ok(Some(merged))
    }

    fn format_change(change: &SemanticChange) -> String {
        match change {
            SemanticChange::Added { path, value } => {
                format!("  ADDED     {path}: {value}")
            }
            SemanticChange::Removed { path, old_value } => {
                format!("  REMOVED   {path}: {old_value}")
            }
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } => {
                format!("  MODIFIED  {path}: {old_value} -> {new_value}")
            }
            SemanticChange::Moved {
                old_path,
                new_path,
                value,
            } => {
                format!("  MOVED     {old_path} -> {new_path}: {value}")
            }
        }
    }
}

fn _merged_components_from_props(
    merged: &mut Vec<String>,
    comp_type: &str,
    merged_props: &[(String, String)],
) {
    let uid = merged_props
        .iter()
        .find(|(k, _)| k == "UID")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    let uid_key = format!("{comp_type}[UID={uid}]");
    if !merged.contains(&uid_key) {
        merged.push(uid_key);
    }
}

impl Default for IcalDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for IcalDriver {
    fn name(&self) -> &str {
        "ICAL"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".ics", ".ifb"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_components = Self::parse_ical(new_content)?;

        match base_content {
            None => {
                let mut changes = Vec::new();
                let inner = Self::extract_inner_components(&new_components);
                for (comp_type, props) in &inner {
                    let uid = Self::extract_uid(props).unwrap_or_else(|| "?".to_string());
                    let base_path = format!("/VCALENDAR/{comp_type}[UID={uid}]");
                    for (key, value) in props {
                        changes.push(SemanticChange::Added {
                            path: format!("{base_path}/{key}"),
                            value: value.clone(),
                        });
                    }
                }
                Ok(changes)
            }
            Some(base) => {
                let old_components = Self::parse_ical(base)?;
                let old_inner = Self::extract_inner_components(&old_components);
                let new_inner = Self::extract_inner_components(&new_components);

                let old_by_uid = Self::components_by_uid(&old_inner);
                let new_by_uid = Self::components_by_uid(&new_inner);

                let mut changes = Vec::new();
                let all_keys: BTreeSet<&String> =
                    old_by_uid.keys().chain(new_by_uid.keys()).collect();

                for key in &all_keys {
                    let in_old = old_by_uid.contains_key(*key);
                    let in_new = new_by_uid.contains_key(*key);

                    match (in_old, in_new) {
                        (true, false) => {
                            let props = &old_by_uid[*key];
                            changes.push(SemanticChange::Removed {
                                path: format!("/VCALENDAR/{key}"),
                                old_value: format!(
                                    "\"{}\"",
                                    Self::extract_uid(props).unwrap_or_default()
                                ),
                            });
                        }
                        (false, true) => {
                            let props = &new_by_uid[*key];
                            changes.push(SemanticChange::Added {
                                path: format!("/VCALENDAR/{key}"),
                                value: format!(
                                    "\"{}\"",
                                    Self::extract_uid(props).as_deref().unwrap_or("?")
                                ),
                            });
                        }
                        (true, true) => {
                            let old_props = &old_by_uid[*key];
                            let new_props = &new_by_uid[*key];
                            changes.extend(Self::diff_properties(key, "", old_props, new_props));
                        }
                        (false, false) => {}
                    }
                }

                Ok(changes)
            }
        }
    }

    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<String, DriverError> {
        let changes = self.diff(base_content, new_content)?;

        if changes.is_empty() {
            return Ok("no changes".to_string());
        }

        let lines: Vec<String> = changes.iter().map(Self::format_change).collect();
        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_components = Self::parse_ical(base)?;
        let ours_components = Self::parse_ical(ours)?;
        let theirs_components = Self::parse_ical(theirs)?;

        let base_inner = Self::extract_inner_components(&base_components);
        let ours_inner = Self::extract_inner_components(&ours_components);
        let theirs_inner = Self::extract_inner_components(&theirs_components);

        match Self::merge_components(&base_inner, &ours_inner, &theirs_inner)? {
            Some(merged) => Ok(Some(Self::serialize_components(&merged))),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_ICAL: &str = "BEGIN:VCALENDAR\r\n\
        VERSION:2.0\r\n\
        PRODID:-//Test//EN\r\n\
        BEGIN:VEVENT\r\n\
        DTSTART:20240101T100000Z\r\n\
        DTEND:20240101T110000Z\r\n\
        SUMMARY:Team Meeting\r\n\
        LOCATION:Room 101\r\n\
        UID:abc123@example.com\r\n\
        END:VEVENT\r\n\
        BEGIN:VEVENT\r\n\
        DTSTART:20240102T090000Z\r\n\
        DTEND:20240102T100000Z\r\n\
        SUMMARY:Standup\r\n\
        UID:def456@example.com\r\n\
        END:VEVENT\r\n\
        END:VCALENDAR\r\n";

    #[test]
    fn test_new_ics_file() {
        let driver = IcalDriver::new();
        let new_content = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VEVENT\r\n\
            DTSTART:20240101T100000Z\r\n\
            SUMMARY:New Event\r\n\
            UID:abc123@example.com\r\n\
            END:VEVENT\r\n\
            END:VCALENDAR\r\n";

        let changes = driver.diff(None, new_content).unwrap();
        assert!(!changes.is_empty());
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path.contains("SUMMARY") && value == "New Event"
        )));
    }

    #[test]
    fn test_single_event_summary_change() {
        let driver = IcalDriver::new();
        let new_content = BASE_ICAL.replace("SUMMARY:Team Meeting", "SUMMARY:Sprint Planning");

        let changes = driver.diff(Some(BASE_ICAL), &new_content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } if path.contains("SUMMARY")
                && old_value == "Team Meeting"
                && new_value == "Sprint Planning"
        )));
    }

    #[test]
    fn test_event_dtstart_change() {
        let driver = IcalDriver::new();
        let new_content = BASE_ICAL.replace("DTSTART:20240101T100000Z", "DTSTART:20240102T100000Z");

        let changes = driver.diff(Some(BASE_ICAL), &new_content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } if path.contains("DTSTART")
                && old_value == "20240101T100000Z"
                && new_value == "20240102T100000Z"
        )));
    }

    #[test]
    fn test_event_location_change() {
        let driver = IcalDriver::new();
        let new_content = BASE_ICAL.replace("LOCATION:Room 101", "LOCATION:Conference Room B");

        let changes = driver.diff(Some(BASE_ICAL), &new_content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } if path.contains("LOCATION")
                && old_value == "Room 101"
                && new_value == "Conference Room B"
        )));
    }

    #[test]
    fn test_new_event_added() {
        let driver = IcalDriver::new();
        let new_content = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VEVENT\r\n\
            DTSTART:20240101T100000Z\r\n\
            DTEND:20240101T110000Z\r\n\
            SUMMARY:Team Meeting\r\n\
            LOCATION:Room 101\r\n\
            UID:abc123@example.com\r\n\
            END:VEVENT\r\n\
            BEGIN:VEVENT\r\n\
            DTSTART:20240102T090000Z\r\n\
            DTEND:20240102T100000Z\r\n\
            SUMMARY:Standup\r\n\
            UID:def456@example.com\r\n\
            END:VEVENT\r\n\
            BEGIN:VEVENT\r\n\
            DTSTART:20240103T140000Z\r\n\
            SUMMARY:Workshop\r\n\
            UID:ghi789@example.com\r\n\
            END:VEVENT\r\n\
            END:VCALENDAR\r\n";

        let changes = driver.diff(Some(BASE_ICAL), &new_content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path.contains("ghi789")
        )));
    }

    #[test]
    fn test_event_removed() {
        let driver = IcalDriver::new();
        let new_content = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VEVENT\r\n\
            DTSTART:20240101T100000Z\r\n\
            DTEND:20240101T110000Z\r\n\
            SUMMARY:Team Meeting\r\n\
            LOCATION:Room 101\r\n\
            UID:abc123@example.com\r\n\
            END:VEVENT\r\n\
            END:VCALENDAR\r\n";

        let changes = driver.diff(Some(BASE_ICAL), &new_content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { path, .. } if path.contains("def456")
        )));
    }

    #[test]
    fn test_attendee_added_to_event() {
        let driver = IcalDriver::new();
        let new_content = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VEVENT\r\n\
            DTSTART:20240101T100000Z\r\n\
            DTEND:20240101T110000Z\r\n\
            SUMMARY:Team Meeting\r\n\
            LOCATION:Room 101\r\n\
            UID:abc123@example.com\r\n\
            ATTENDEE:mailto:bob@example.com\r\n\
            END:VEVENT\r\n\
            BEGIN:VEVENT\r\n\
            DTSTART:20240102T090000Z\r\n\
            DTEND:20240102T100000Z\r\n\
            SUMMARY:Standup\r\n\
            UID:def456@example.com\r\n\
            END:VEVENT\r\n\
            END:VCALENDAR\r\n";

        let changes = driver.diff(Some(BASE_ICAL), &new_content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path.contains("ATTENDEE")
                && value == "mailto:bob@example.com"
        )));
    }

    #[test]
    fn test_vtodo_priority_change() {
        let driver = IcalDriver::new();
        let base = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VTODO\r\n\
            SUMMARY:Review PRs\r\n\
            PRIORITY:5\r\n\
            UID:todo1@example.com\r\n\
            END:VTODO\r\n\
            END:VCALENDAR\r\n";
        let new = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VTODO\r\n\
            SUMMARY:Review PRs\r\n\
            PRIORITY:1\r\n\
            UID:todo1@example.com\r\n\
            END:VTODO\r\n\
            END:VCALENDAR\r\n";

        let changes = driver.diff(Some(base), &new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } if path.contains("PRIORITY")
                && old_value == "5"
                && new_value == "1"
        )));
    }

    #[test]
    fn test_clean_merge_different_events_modified() {
        let driver = IcalDriver::new();
        let ours = BASE_ICAL.replace("SUMMARY:Team Meeting", "SUMMARY:Sprint Planning");
        let theirs = BASE_ICAL.replace("SUMMARY:Standup", "SUMMARY:Daily Sync");

        let result = driver.merge(BASE_ICAL, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Sprint Planning"));
        assert!(merged.contains("Daily Sync"));
    }

    #[test]
    fn test_conflict_merge_same_event_summary_changed() {
        let driver = IcalDriver::new();
        let ours = BASE_ICAL.replace("SUMMARY:Team Meeting", "SUMMARY:Sprint Planning");
        let theirs = BASE_ICAL.replace("SUMMARY:Team Meeting", "SUMMARY:Retrospective");

        let result = driver.merge(BASE_ICAL, &ours, &theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_multiline_description_handling() {
        let driver = IcalDriver::new();
        let base = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VEVENT\r\n\
            SUMMARY:Meeting\r\n\
            UID:multi@example.com\r\n\
            END:VEVENT\r\n\
            END:VCALENDAR\r\n";

        // Note: RFC 5545 fold = CRLF + single space. We build this manually
        // because Rust's \ line continuation strips leading whitespace.
        let new = [
            "BEGIN:VCALENDAR\r\n",
            "VERSION:2.0\r\n",
            "PRODID:-//Test//EN\r\n",
            "BEGIN:VEVENT\r\n",
            "SUMMARY:Meeting\r\n",
            "UID:multi@example.com\r\n",
            "DESCRIPTION:This is a long description that is folded\r\n",
            " across multiple lines as per RFC 5545.\r\n",
            "END:VEVENT\r\n",
            "END:VCALENDAR\r\n",
        ]
        .concat();

        let changes = driver.diff(Some(base), &new).unwrap();
        // RFC 5545 §3.1: the leading space (fold indicator) is removed during unfolding,
        // so "folded\r\n across" unfolds to "foldedacross" (no space between).
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path.contains("DESCRIPTION")
                && value.contains("foldedacross multiple lines")
        )));
    }

    #[test]
    fn test_rrule_modification() {
        let driver = IcalDriver::new();
        let base = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VEVENT\r\n\
            SUMMARY:Weekly Standup\r\n\
            UID:rrule@example.com\r\n\
            RRULE:FREQ=WEEKLY;COUNT=10\r\n\
            END:VEVENT\r\n\
            END:VCALENDAR\r\n";
        let new = "BEGIN:VCALENDAR\r\n\
            VERSION:2.0\r\n\
            PRODID:-//Test//EN\r\n\
            BEGIN:VEVENT\r\n\
            SUMMARY:Weekly Standup\r\n\
            UID:rrule@example.com\r\n\
            RRULE:FREQ=WEEKLY;COUNT=20\r\n\
            END:VEVENT\r\n\
            END:VCALENDAR\r\n";

        let changes = driver.diff(Some(base), &new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } if path.contains("RRULE")
                && old_value == "FREQ=WEEKLY;COUNT=10"
                && new_value == "FREQ=WEEKLY;COUNT=20"
        )));
    }

    #[test]
    fn test_driver_name() {
        let driver = IcalDriver::new();
        assert_eq!(driver.name(), "ICAL");
    }

    #[test]
    fn test_driver_extensions() {
        let driver = IcalDriver::new();
        assert_eq!(driver.supported_extensions(), &[".ics", ".ifb"]);
    }

    #[test]
    fn test_format_diff_no_changes() {
        let driver = IcalDriver::new();
        let result = driver.format_diff(Some(BASE_ICAL), BASE_ICAL).unwrap();
        assert_eq!(result, "no changes");
    }
}
