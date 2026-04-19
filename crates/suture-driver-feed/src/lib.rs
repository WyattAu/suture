use std::collections::HashMap;

use suture_driver::{DriverError, SemanticChange, SutureDriver};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedType {
    Rss,
    Atom,
}

#[derive(Debug, Clone, PartialEq)]
struct FeedEntry {
    id: String,
    title: String,
    link: String,
    description: String,
    pub_date: String,
    author: String,
    categories: Vec<String>,
    content: String,
}

#[derive(Debug, Clone)]
struct FeedMetadata {
    title: String,
    description: String,
    link: String,
    language: String,
    copyright: String,
    managing_editor: String,
}

#[derive(Debug, Clone)]
struct ParsedFeed {
    feed_type: FeedType,
    metadata: FeedMetadata,
    entries: Vec<FeedEntry>,
}

pub struct FeedDriver;

impl FeedDriver {
    pub fn new() -> Self {
        Self
    }

    fn detect_feed_type(doc: &roxmltree::Document) -> FeedType {
        let root = doc.root_element();
        let tag = root.tag_name().name();
        if tag == "rss" {
            FeedType::Rss
        } else {
            FeedType::Atom
        }
    }

    fn text_content(node: roxmltree::Node, tag: &str) -> String {
        node.children()
            .find(|n| n.is_element() && n.tag_name().name() == tag)
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn attr_value(node: roxmltree::Node, tag: &str, attr: &str) -> String {
        node.children()
            .find(|n| n.is_element() && n.tag_name().name() == tag)
            .and_then(|n| n.attribute(attr))
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn parse_rss(doc: &roxmltree::Document) -> ParsedFeed {
        let root = doc.root_element();
        let channel = root
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "channel");

        let (metadata, channel_node) = match channel {
            Some(ch) => (
                FeedMetadata {
                    title: Self::text_content(ch, "title"),
                    description: Self::text_content(ch, "description"),
                    link: Self::text_content(ch, "link"),
                    language: Self::text_content(ch, "language"),
                    copyright: Self::text_content(ch, "copyright"),
                    managing_editor: Self::text_content(ch, "managingEditor"),
                },
                ch,
            ),
            None => (
                FeedMetadata {
                    title: String::new(),
                    description: String::new(),
                    link: String::new(),
                    language: String::new(),
                    copyright: String::new(),
                    managing_editor: String::new(),
                },
                root,
            ),
        };

        let entries: Vec<FeedEntry> = channel_node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "item")
            .map(|item| {
                let categories: Vec<String> = item
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "category")
                    .filter_map(|n| n.text().map(|t| t.trim().to_string()))
                    .filter(|t| !t.is_empty())
                    .collect();

                FeedEntry {
                    id: Self::text_content(item, "guid"),
                    title: Self::text_content(item, "title"),
                    link: Self::text_content(item, "link"),
                    description: Self::text_content(item, "description"),
                    pub_date: Self::text_content(item, "pubDate"),
                    author: Self::text_content(item, "author"),
                    content: Self::text_content(item, "content:encoded"),
                    categories,
                }
            })
            .collect();

        ParsedFeed {
            feed_type: FeedType::Rss,
            metadata,
            entries,
        }
    }

    fn parse_atom(doc: &roxmltree::Document) -> ParsedFeed {
        let root = doc.root_element();

        let metadata = FeedMetadata {
            title: Self::text_content(root, "title"),
            description: Self::text_content(root, "subtitle"),
            link: Self::attr_value(root, "link", "href"),
            language: String::new(),
            copyright: Self::text_content(root, "rights"),
            managing_editor: String::new(),
        };

        let entries: Vec<FeedEntry> = root
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "entry")
            .map(|entry| {
                let author_name = entry
                    .children()
                    .find(|n| n.is_element() && n.tag_name().name() == "author")
                    .and_then(|a| {
                        a.children()
                            .find(|n| n.is_element() && n.tag_name().name() == "name")
                    })
                    .and_then(|n| n.text())
                    .unwrap_or("")
                    .trim()
                    .to_string();

                let categories: Vec<String> = entry
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "category")
                    .filter_map(|n| n.attribute("term").map(|t| t.trim().to_string()))
                    .filter(|t| !t.is_empty())
                    .collect();

                FeedEntry {
                    id: Self::text_content(entry, "id"),
                    title: Self::text_content(entry, "title"),
                    link: Self::attr_value(entry, "link", "href"),
                    description: Self::text_content(entry, "summary"),
                    pub_date: Self::text_content(entry, "updated"),
                    author: author_name,
                    content: Self::text_content(entry, "content"),
                    categories,
                }
            })
            .collect();

        ParsedFeed {
            feed_type: FeedType::Atom,
            metadata,
            entries,
        }
    }

    fn parse(content: &str) -> Result<ParsedFeed, DriverError> {
        let doc = roxmltree::Document::parse(content)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;
        let feed_type = Self::detect_feed_type(&doc);
        match feed_type {
            FeedType::Rss => Ok(Self::parse_rss(&doc)),
            FeedType::Atom => Ok(Self::parse_atom(&doc)),
        }
    }

    fn type_label(feed_type: FeedType) -> &'static str {
        match feed_type {
            FeedType::Rss => "RSS 2.0",
            FeedType::Atom => "Atom",
        }
    }

    fn entry_path(feed_type: FeedType, id: &str) -> String {
        match feed_type {
            FeedType::Rss => format!("/channel/item[guid={id}]"),
            FeedType::Atom => format!("/feed/entry[id={id}]"),
        }
    }

    fn diff_metadata(
        feed_type: FeedType,
        old: &FeedMetadata,
        new: &FeedMetadata,
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        let prefix = match feed_type {
            FeedType::Rss => "/channel",
            FeedType::Atom => "/feed",
        };

        if old.title != new.title {
            changes.push(SemanticChange::Modified {
                path: format!("{prefix}/title"),
                old_value: old.title.clone(),
                new_value: new.title.clone(),
            });
        }

        let desc_tag = match feed_type {
            FeedType::Rss => "description",
            FeedType::Atom => "subtitle",
        };
        if old.description != new.description {
            changes.push(SemanticChange::Modified {
                path: format!("{prefix}/{desc_tag}"),
                old_value: old.description.clone(),
                new_value: new.description.clone(),
            });
        }

        if old.link != new.link {
            changes.push(SemanticChange::Modified {
                path: format!("{prefix}/link"),
                old_value: old.link.clone(),
                new_value: new.link.clone(),
            });
        }

        if old.language != new.language && feed_type == FeedType::Rss {
            changes.push(SemanticChange::Modified {
                path: format!("{prefix}/language"),
                old_value: old.language.clone(),
                new_value: new.language.clone(),
            });
        }

        if old.copyright != new.copyright {
            let copy_tag = match feed_type {
                FeedType::Rss => "copyright",
                FeedType::Atom => "rights",
            };
            changes.push(SemanticChange::Modified {
                path: format!("{prefix}/{copy_tag}"),
                old_value: old.copyright.clone(),
                new_value: new.copyright.clone(),
            });
        }

        if old.managing_editor != new.managing_editor && feed_type == FeedType::Rss {
            changes.push(SemanticChange::Modified {
                path: format!("{prefix}/managingEditor"),
                old_value: old.managing_editor.clone(),
                new_value: new.managing_editor.clone(),
            });
        }

        changes
    }

    fn diff_entries(
        feed_type: FeedType,
        old_entries: &[FeedEntry],
        new_entries: &[FeedEntry],
    ) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        let old_map: HashMap<&str, &FeedEntry> =
            old_entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let new_map: HashMap<&str, &FeedEntry> =
            new_entries.iter().map(|e| (e.id.as_str(), e)).collect();

        let old_ids: std::collections::HashSet<&str> = old_map.keys().copied().collect();
        let new_ids: std::collections::HashSet<&str> = new_map.keys().copied().collect();

        for id in &old_ids {
            if !new_ids.contains(id) {
                let entry = old_map[*id];
                changes.push(SemanticChange::Removed {
                    path: Self::entry_path(feed_type, id),
                    old_value: entry.title.clone(),
                });
            }
        }

        for id in &new_ids {
            if !old_ids.contains(id) {
                let entry = new_map[*id];
                changes.push(SemanticChange::Added {
                    path: Self::entry_path(feed_type, id),
                    value: entry.title.clone(),
                });
            }
        }

        for id in old_ids.intersection(&new_ids) {
            let old_entry = old_map[*id];
            let new_entry = new_map[*id];
            let base_path = Self::entry_path(feed_type, id);

            if old_entry.title != new_entry.title {
                changes.push(SemanticChange::Modified {
                    path: format!("{base_path}/title"),
                    old_value: old_entry.title.clone(),
                    new_value: new_entry.title.clone(),
                });
            }

            if old_entry.link != new_entry.link {
                changes.push(SemanticChange::Modified {
                    path: format!("{base_path}/link"),
                    old_value: old_entry.link.clone(),
                    new_value: new_entry.link.clone(),
                });
            }

            let desc_field = match feed_type {
                FeedType::Rss => "description",
                FeedType::Atom => "summary",
            };
            if old_entry.description != new_entry.description {
                changes.push(SemanticChange::Modified {
                    path: format!("{base_path}/{desc_field}"),
                    old_value: old_entry.description.clone(),
                    new_value: new_entry.description.clone(),
                });
            }

            let date_field = match feed_type {
                FeedType::Rss => "pubDate",
                FeedType::Atom => "updated",
            };
            if old_entry.pub_date != new_entry.pub_date {
                changes.push(SemanticChange::Modified {
                    path: format!("{base_path}/{date_field}"),
                    old_value: old_entry.pub_date.clone(),
                    new_value: new_entry.pub_date.clone(),
                });
            }

            if old_entry.author != new_entry.author {
                changes.push(SemanticChange::Modified {
                    path: format!("{base_path}/author"),
                    old_value: old_entry.author.clone(),
                    new_value: new_entry.author.clone(),
                });
            }

            let old_cats: std::collections::HashSet<&str> =
                old_entry.categories.iter().map(|s| s.as_str()).collect();
            let new_cats: std::collections::HashSet<&str> =
                new_entry.categories.iter().map(|s| s.as_str()).collect();

            for cat in &old_cats {
                if !new_cats.contains(cat) {
                    changes.push(SemanticChange::Removed {
                        path: format!("{base_path}/category"),
                        old_value: (*cat).to_string(),
                    });
                }
            }

            for cat in &new_cats {
                if !old_cats.contains(cat) {
                    changes.push(SemanticChange::Added {
                        path: format!("{base_path}/category"),
                        value: (*cat).to_string(),
                    });
                }
            }

            if old_entry.content != new_entry.content {
                changes.push(SemanticChange::Modified {
                    path: format!("{base_path}/content"),
                    old_value: old_entry.content.clone(),
                    new_value: new_entry.content.clone(),
                });
            }
        }

        changes
    }

    fn collect_new_feed(feed: &ParsedFeed) -> Vec<SemanticChange> {
        let mut changes = Vec::new();
        let label = Self::type_label(feed.feed_type);
        let prefix = match feed.feed_type {
            FeedType::Rss => "/channel",
            FeedType::Atom => "/feed",
        };

        changes.push(SemanticChange::Added {
            path: format!("{prefix}/[feed-type]"),
            value: label.to_string(),
        });

        changes.push(SemanticChange::Added {
            path: format!("{prefix}/title"),
            value: feed.metadata.title.clone(),
        });

        changes.push(SemanticChange::Added {
            path: format!("{prefix}/description"),
            value: feed.metadata.description.clone(),
        });

        changes.push(SemanticChange::Added {
            path: format!("{prefix}/link"),
            value: feed.metadata.link.clone(),
        });

        for entry in &feed.entries {
            let path = Self::entry_path(feed.feed_type, &entry.id);
            changes.push(SemanticChange::Added {
                path,
                value: entry.title.clone(),
            });
        }

        changes
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

    fn merge_metadata(
        _feed_type: FeedType,
        base: &FeedMetadata,
        ours: &FeedMetadata,
        theirs: &FeedMetadata,
    ) -> Result<Option<FeedMetadata>, DriverError> {
        let merge_field = |base_val: &str, ours_val: &str, theirs_val: &str| -> Option<String> {
            if ours_val == theirs_val {
                Some(ours_val.to_string())
            } else if ours_val == base_val {
                Some(theirs_val.to_string())
            } else if theirs_val == base_val {
                Some(ours_val.to_string())
            } else {
                None
            }
        };

        let title = merge_field(&base.title, &ours.title, &theirs.title);
        let description = merge_field(&base.description, &ours.description, &theirs.description);
        let link = merge_field(&base.link, &ours.link, &theirs.link);
        let language = merge_field(&base.language, &ours.language, &theirs.language);
        let copyright = merge_field(&base.copyright, &ours.copyright, &theirs.copyright);
        let managing_editor = merge_field(
            &base.managing_editor,
            &ours.managing_editor,
            &theirs.managing_editor,
        );

        match (
            title,
            description,
            link,
            language,
            copyright,
            managing_editor,
        ) {
            (
                Some(title),
                Some(description),
                Some(link),
                Some(language),
                Some(copyright),
                Some(managing_editor),
            ) => Ok(Some(FeedMetadata {
                title,
                description,
                link,
                language,
                copyright,
                managing_editor,
            })),
            _ => Ok(None),
        }
    }

    fn merge_entries(
        feed_type: FeedType,
        base_entries: &[FeedEntry],
        ours_entries: &[FeedEntry],
        theirs_entries: &[FeedEntry],
    ) -> Result<Option<Vec<FeedEntry>>, DriverError> {
        let base_map: HashMap<&str, &FeedEntry> =
            base_entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let ours_map: HashMap<&str, &FeedEntry> =
            ours_entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let theirs_map: HashMap<&str, &FeedEntry> =
            theirs_entries.iter().map(|e| (e.id.as_str(), e)).collect();

        let base_ids: std::collections::HashSet<&str> = base_map.keys().copied().collect();
        let ours_ids: std::collections::HashSet<&str> = ours_map.keys().copied().collect();
        let theirs_ids: std::collections::HashSet<&str> = theirs_map.keys().copied().collect();

        let all_ids: std::collections::HashSet<&str> = base_ids
            .iter()
            .chain(ours_ids.iter())
            .chain(theirs_ids.iter())
            .copied()
            .collect();

        let mut merged_entries: Vec<FeedEntry> = Vec::new();

        for id in &all_ids {
            let in_base = base_ids.contains(id);
            let in_ours = ours_ids.contains(id);
            let in_theirs = theirs_ids.contains(id);

            match (in_base, in_ours, in_theirs) {
                (true, true, true) => {
                    let base_e = base_map[*id];
                    let ours_e = ours_map[*id];
                    let theirs_e = theirs_map[*id];

                    let merged = Self::merge_entry(feed_type, base_e, ours_e, theirs_e)?;
                    if let Some(e) = merged {
                        merged_entries.push(e);
                    } else {
                        return Ok(None);
                    }
                }
                (true, true, false) => {
                    merged_entries.push((*ours_map[*id]).clone());
                }
                (true, false, true) => {
                    merged_entries.push((*theirs_map[*id]).clone());
                }
                (true, false, false) => {
                    continue;
                }
                (false, true, true) => {
                    let ours_e = ours_map[*id];
                    let theirs_e = theirs_map[*id];
                    if ours_e == theirs_e {
                        merged_entries.push((*ours_e).clone());
                    } else {
                        return Ok(None);
                    }
                }
                (false, true, false) => {
                    merged_entries.push((*ours_map[*id]).clone());
                }
                (false, false, true) => {
                    merged_entries.push((*theirs_map[*id]).clone());
                }
                (false, false, false) => {}
            }
        }

        Ok(Some(merged_entries))
    }

    fn merge_entry(
        _feed_type: FeedType,
        base: &FeedEntry,
        ours: &FeedEntry,
        theirs: &FeedEntry,
    ) -> Result<Option<FeedEntry>, DriverError> {
        let merge_field = |base_val: &str, ours_val: &str, theirs_val: &str| -> Option<String> {
            if ours_val == theirs_val {
                Some(ours_val.to_string())
            } else if ours_val == base_val {
                Some(theirs_val.to_string())
            } else if theirs_val == base_val {
                Some(ours_val.to_string())
            } else {
                None
            }
        };

        let title = merge_field(&base.title, &ours.title, &theirs.title);
        let link = merge_field(&base.link, &ours.link, &theirs.link);
        let description = merge_field(&base.description, &ours.description, &theirs.description);
        let pub_date = merge_field(&base.pub_date, &ours.pub_date, &theirs.pub_date);
        let author = merge_field(&base.author, &ours.author, &theirs.author);
        let content = merge_field(&base.content, &ours.content, &theirs.content);

        match (title, link, description, pub_date, author, content) {
            (
                Some(title),
                Some(link),
                Some(description),
                Some(pub_date),
                Some(author),
                Some(content),
            ) => {
                let base_cats: std::collections::HashSet<&str> =
                    base.categories.iter().map(|s| s.as_str()).collect();
                let ours_cats: std::collections::HashSet<&str> =
                    ours.categories.iter().map(|s| s.as_str()).collect();
                let theirs_cats: std::collections::HashSet<&str> =
                    theirs.categories.iter().map(|s| s.as_str()).collect();

                let mut merged_cats: Vec<String> = Vec::new();
                let all_cats: std::collections::HashSet<&str> = base_cats
                    .iter()
                    .chain(ours_cats.iter())
                    .chain(theirs_cats.iter())
                    .copied()
                    .collect();

                for cat in &all_cats {
                    let in_base = base_cats.contains(cat);
                    let in_ours = ours_cats.contains(cat);
                    let in_theirs = theirs_cats.contains(cat);

                    match (in_base, in_ours, in_theirs) {
                        (_, true, true) | (_, true, false) | (_, false, true) => {
                            merged_cats.push((*cat).to_string());
                        }
                        (true, false, false) => {}
                        (false, false, false) => {}
                    }
                }

                merged_cats.sort();

                Ok(Some(FeedEntry {
                    id: base.id.clone(),
                    title,
                    link,
                    description,
                    pub_date,
                    author,
                    content,
                    categories: merged_cats,
                }))
            }
            _ => Ok(None),
        }
    }

    fn feed_to_string(feed: &ParsedFeed) -> String {
        match feed.feed_type {
            FeedType::Rss => Self::rss_to_string(feed),
            FeedType::Atom => Self::atom_to_string(feed),
        }
    }

    fn rss_to_string(feed: &ParsedFeed) -> String {
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        s.push_str("<rss version=\"2.0\">\n");
        s.push_str("  <channel>\n");

        s.push_str(&format!(
            "    <title>{}</title>\n",
            escape_xml(&feed.metadata.title)
        ));
        s.push_str(&format!(
            "    <link>{}</link>\n",
            escape_xml(&feed.metadata.link)
        ));
        s.push_str(&format!(
            "    <description>{}</description>\n",
            escape_xml(&feed.metadata.description)
        ));

        if !feed.metadata.language.is_empty() {
            s.push_str(&format!(
                "    <language>{}</language>\n",
                escape_xml(&feed.metadata.language)
            ));
        }
        if !feed.metadata.copyright.is_empty() {
            s.push_str(&format!(
                "    <copyright>{}</copyright>\n",
                escape_xml(&feed.metadata.copyright)
            ));
        }
        if !feed.metadata.managing_editor.is_empty() {
            s.push_str(&format!(
                "    <managingEditor>{}</managingEditor>\n",
                escape_xml(&feed.metadata.managing_editor)
            ));
        }

        for entry in &feed.entries {
            s.push_str("    <item>\n");
            if !entry.title.is_empty() {
                s.push_str(&format!(
                    "      <title>{}</title>\n",
                    escape_xml(&entry.title)
                ));
            }
            if !entry.link.is_empty() {
                s.push_str(&format!("      <link>{}</link>\n", escape_xml(&entry.link)));
            }
            if !entry.description.is_empty() {
                s.push_str(&format!(
                    "      <description>{}</description>\n",
                    escape_xml(&entry.description)
                ));
            }
            if !entry.pub_date.is_empty() {
                s.push_str(&format!(
                    "      <pubDate>{}</pubDate>\n",
                    escape_xml(&entry.pub_date)
                ));
            }
            if !entry.id.is_empty() {
                s.push_str(&format!("      <guid>{}</guid>\n", escape_xml(&entry.id)));
            }
            for cat in &entry.categories {
                s.push_str(&format!("      <category>{}</category>\n", escape_xml(cat)));
            }
            if !entry.author.is_empty() {
                s.push_str(&format!(
                    "      <author>{}</author>\n",
                    escape_xml(&entry.author)
                ));
            }
            if !entry.content.is_empty() {
                s.push_str(&format!(
                    "      <content:encoded>{}</content:encoded>\n",
                    escape_xml(&entry.content)
                ));
            }
            s.push_str("    </item>\n");
        }

        s.push_str("  </channel>\n");
        s.push_str("</rss>\n");
        s
    }

    fn atom_to_string(feed: &ParsedFeed) -> String {
        let mut s = String::new();
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        s.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");

        s.push_str(&format!(
            "  <title>{}</title>\n",
            escape_xml(&feed.metadata.title)
        ));
        if !feed.metadata.link.is_empty() {
            s.push_str(&format!(
                "  <link href=\"{}\"/>\n",
                escape_xml(&feed.metadata.link)
            ));
        }
        if !feed.metadata.description.is_empty() {
            s.push_str(&format!(
                "  <subtitle>{}</subtitle>\n",
                escape_xml(&feed.metadata.description)
            ));
        }

        for entry in &feed.entries {
            s.push_str("  <entry>\n");
            if !entry.title.is_empty() {
                s.push_str(&format!(
                    "    <title>{}</title>\n",
                    escape_xml(&entry.title)
                ));
            }
            if !entry.link.is_empty() {
                s.push_str(&format!(
                    "    <link href=\"{}\"/>\n",
                    escape_xml(&entry.link)
                ));
            }
            if !entry.description.is_empty() {
                s.push_str(&format!(
                    "      <summary>{}</summary>\n",
                    escape_xml(&entry.description)
                ));
            }
            if !entry.pub_date.is_empty() {
                s.push_str(&format!(
                    "    <updated>{}</updated>\n",
                    escape_xml(&entry.pub_date)
                ));
            }
            if !entry.id.is_empty() {
                s.push_str(&format!("    <id>{}</id>\n", escape_xml(&entry.id)));
            }
            for cat in &entry.categories {
                s.push_str(&format!("    <category term=\"{}\"/>\n", escape_xml(cat)));
            }
            if !entry.author.is_empty() {
                s.push_str("    <author>\n");
                s.push_str(&format!(
                    "      <name>{}</name>\n",
                    escape_xml(&entry.author)
                ));
                s.push_str("    </author>\n");
            }
            if !entry.content.is_empty() {
                s.push_str(&format!(
                    "    <content>{}</content>\n",
                    escape_xml(&entry.content)
                ));
            }
            s.push_str("  </entry>\n");
        }

        s.push_str("</feed>\n");
        s
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl Default for FeedDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for FeedDriver {
    fn name(&self) -> &str {
        "FEED"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".rss", ".atom"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_feed = Self::parse(new_content)?;

        match base_content {
            None => Ok(Self::collect_new_feed(&new_feed)),
            Some(base) => {
                let old_feed = Self::parse(base)?;

                if old_feed.feed_type != new_feed.feed_type {
                    return Err(DriverError::ParseError(format!(
                        "feed type mismatch: base is {} but new is {}",
                        Self::type_label(old_feed.feed_type),
                        Self::type_label(new_feed.feed_type),
                    )));
                }

                let mut changes = Vec::new();
                changes.extend(Self::diff_metadata(
                    new_feed.feed_type,
                    &old_feed.metadata,
                    &new_feed.metadata,
                ));
                changes.extend(Self::diff_entries(
                    new_feed.feed_type,
                    &old_feed.entries,
                    &new_feed.entries,
                ));
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
        let base_feed = Self::parse(base)?;
        let ours_feed = Self::parse(ours)?;
        let theirs_feed = Self::parse(theirs)?;

        if ours_feed.feed_type != theirs_feed.feed_type {
            return Err(DriverError::ParseError(format!(
                "cannot merge {} with {}",
                Self::type_label(ours_feed.feed_type),
                Self::type_label(theirs_feed.feed_type),
            )));
        }

        let feed_type = ours_feed.feed_type;

        let merged_metadata = Self::merge_metadata(
            feed_type,
            &base_feed.metadata,
            &ours_feed.metadata,
            &theirs_feed.metadata,
        )?;

        let merged_entries = Self::merge_entries(
            feed_type,
            &base_feed.entries,
            &ours_feed.entries,
            &theirs_feed.entries,
        )?;

        match (merged_metadata, merged_entries) {
            (Some(metadata), Some(entries)) => {
                let feed = ParsedFeed {
                    feed_type,
                    metadata,
                    entries,
                };
                Ok(Some(Self::feed_to_string(&feed)))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS_BASE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>My Feed</title>
    <link>https://example.com</link>
    <description>A feed about things</description>
    <item>
      <title>Article 1</title>
      <link>https://example.com/article1</link>
      <description>Description 1</description>
      <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
      <guid>abc123</guid>
      <category>tech</category>
      <author>author@example.com</author>
    </item>
    <item>
      <title>Article 2</title>
      <link>https://example.com/article2</link>
      <description>Description 2</description>
      <pubDate>Tue, 02 Jan 2024 00:00:00 GMT</pubDate>
      <guid>def456</guid>
      <category>science</category>
      <author>author2@example.com</author>
    </item>
  </channel>
</rss>"#;

    const ATOM_BASE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom Feed</title>
  <link href="https://example.com"/>
  <subtitle>An atom feed</subtitle>
  <entry>
    <title>Entry 1</title>
    <link href="https://example.com/entry1"/>
    <summary>Summary 1</summary>
    <updated>2024-01-01T00:00:00Z</updated>
    <id>atom-1</id>
    <category term="tech"/>
    <author><name>Author One</name></author>
  </entry>
  <entry>
    <title>Entry 2</title>
    <link href="https://example.com/entry2"/>
    <summary>Summary 2</summary>
    <updated>2024-01-02T00:00:00Z</updated>
    <id>atom-2</id>
    <author><name>Author Two</name></author>
  </entry>
</feed>"#;

    #[test]
    fn test_new_rss_file() {
        let driver = FeedDriver::new();
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>New Feed</title>
    <link>https://example.com</link>
    <description>A new feed</description>
    <item>
      <title>First Post</title>
      <guid>post-1</guid>
    </item>
  </channel>
</rss>"#;

        let changes = driver.diff(None, content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path == "/channel/[feed-type]" && value == "RSS 2.0"
        )));
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path == "/channel/title" && value == "New Feed"
        )));
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path == "/channel/item[guid=post-1]" && value == "First Post"
        )));
    }

    #[test]
    fn test_new_atom_file() {
        let driver = FeedDriver::new();
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>New Atom Feed</title>
  <entry>
    <title>First Entry</title>
    <id>entry-1</id>
  </entry>
</feed>"#;

        let changes = driver.diff(None, content).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path == "/feed/[feed-type]" && value == "Atom"
        )));
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path == "/feed/title" && value == "New Atom Feed"
        )));
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value } if path == "/feed/entry[id=entry-1]" && value == "First Entry"
        )));
    }

    #[test]
    fn test_rss_entry_title_change() {
        let driver = FeedDriver::new();
        let new_content = RSS_BASE.replace("Article 1", "Updated Article 1");

        let changes = driver.diff(Some(RSS_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/channel/item[guid=abc123]/title".to_string(),
            old_value: "Article 1".to_string(),
            new_value: "Updated Article 1".to_string(),
        }));
    }

    #[test]
    fn test_rss_entry_link_change() {
        let driver = FeedDriver::new();
        let new_content = RSS_BASE.replace(
            "https://example.com/article1",
            "https://example.com/article1-v2",
        );

        let changes = driver.diff(Some(RSS_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/channel/item[guid=abc123]/link".to_string(),
            old_value: "https://example.com/article1".to_string(),
            new_value: "https://example.com/article1-v2".to_string(),
        }));
    }

    #[test]
    fn test_rss_new_entry_added() {
        let driver = FeedDriver::new();
        let new_content = RSS_BASE.replace(
            "</channel>",
            "    <item>\n      <title>Article 3</title>\n      <guid>ghi789</guid>\n    </item>\n  </channel>",
        );

        let changes = driver.diff(Some(RSS_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Added {
            path: "/channel/item[guid=ghi789]".to_string(),
            value: "Article 3".to_string(),
        }));
    }

    #[test]
    fn test_rss_entry_removed() {
        let driver = FeedDriver::new();
        let mut new_content = RSS_BASE.to_string();
        let start = new_content
            .find("    <item>\n      <title>Article 2")
            .unwrap();
        let end = new_content.find("</channel>").unwrap();
        new_content.replace_range(start..end, "");

        let changes = driver.diff(Some(RSS_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Removed {
            path: "/channel/item[guid=def456]".to_string(),
            old_value: "Article 2".to_string(),
        }));
    }

    #[test]
    fn test_atom_entry_title_change() {
        let driver = FeedDriver::new();
        let new_content = ATOM_BASE.replace("Entry 1", "Updated Entry 1");

        let changes = driver.diff(Some(ATOM_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/feed/entry[id=atom-1]/title".to_string(),
            old_value: "Entry 1".to_string(),
            new_value: "Updated Entry 1".to_string(),
        }));
    }

    #[test]
    fn test_atom_entry_summary_change() {
        let driver = FeedDriver::new();
        let new_content = ATOM_BASE.replace("Summary 1", "Updated Summary 1");

        let changes = driver.diff(Some(ATOM_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/feed/entry[id=atom-1]/summary".to_string(),
            old_value: "Summary 1".to_string(),
            new_value: "Updated Summary 1".to_string(),
        }));
    }

    #[test]
    fn test_rss_feed_title_change() {
        let driver = FeedDriver::new();
        let new_content = RSS_BASE.replace("My Feed", "Updated Feed");

        let changes = driver.diff(Some(RSS_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/channel/title".to_string(),
            old_value: "My Feed".to_string(),
            new_value: "Updated Feed".to_string(),
        }));
    }

    #[test]
    fn test_rss_clean_merge() {
        let driver = FeedDriver::new();
        let ours = RSS_BASE.replace("Article 1", "Modified Article 1");
        let theirs = RSS_BASE.replace("Article 2", "Modified Article 2");

        let result = driver.merge(RSS_BASE, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Modified Article 1"));
        assert!(merged.contains("Modified Article 2"));
        assert!(merged.contains("Description 1"));
        assert!(merged.contains("Description 2"));
    }

    #[test]
    fn test_rss_conflict_merge() {
        let driver = FeedDriver::new();
        let ours = RSS_BASE.replace("Article 1", "Ours Title");
        let theirs = RSS_BASE.replace("Article 1", "Theirs Title");

        let result = driver.merge(RSS_BASE, &ours, &theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_atom_clean_merge() {
        let driver = FeedDriver::new();
        let ours = ATOM_BASE.replace("Entry 1", "Modified Entry 1");
        let theirs = ATOM_BASE.replace("Entry 2", "Modified Entry 2");

        let result = driver.merge(ATOM_BASE, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Modified Entry 1"));
        assert!(merged.contains("Modified Entry 2"));
    }

    #[test]
    fn test_entry_added_by_one_removed_by_other_conflict() {
        let driver = FeedDriver::new();
        let base = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>My Feed</title>
    <item>
      <title>Shared</title>
      <guid>shared-1</guid>
    </item>
    <item>
      <title>Will Be Removed</title>
      <guid>remove-me</guid>
    </item>
  </channel>
</rss>"#;

        let ours = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>My Feed</title>
    <item>
      <title>Shared</title>
      <guid>shared-1</guid>
    </item>
    <item>
      <title>Added By Ours</title>
      <guid>added-ours</guid>
    </item>
  </channel>
</rss>"#;

        let theirs = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>My Feed</title>
    <item>
      <title>Shared</title>
      <guid>shared-1</guid>
    </item>
    <item>
      <title>Added By Theirs</title>
      <guid>added-theirs</guid>
    </item>
    <item>
      <title>Will Be Removed</title>
      <guid>remove-me</guid>
    </item>
  </channel>
</rss>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Added By Ours"));
        assert!(merged.contains("Added By Theirs"));
        assert!(
            merged.contains("remove-me"),
            "entry present in base+theirs should be kept"
        );
    }

    #[test]
    fn test_category_addition_to_entry() {
        let driver = FeedDriver::new();
        let new_content = RSS_BASE.replace(
            "<category>tech</category>",
            "<category>tech</category>\n      <category>programming</category>",
        );

        let changes = driver.diff(Some(RSS_BASE), &new_content).unwrap();
        assert!(changes.contains(&SemanticChange::Added {
            path: "/channel/item[guid=abc123]/category".to_string(),
            value: "programming".to_string(),
        }));
    }

    #[test]
    fn test_driver_name() {
        let driver = FeedDriver::new();
        assert_eq!(driver.name(), "FEED");
    }

    #[test]
    fn test_driver_extensions() {
        let driver = FeedDriver::new();
        assert_eq!(driver.supported_extensions(), &[".rss", ".atom"]);
    }

    #[test]
    fn test_format_diff_rss() {
        let driver = FeedDriver::new();
        let new_content = RSS_BASE.replace("Article 1", "Updated Article 1");

        let output = driver.format_diff(Some(RSS_BASE), &new_content).unwrap();
        assert!(output.contains("MODIFIED"));
        assert!(output.contains("abc123"));
        assert!(output.contains("Article 1"));
        assert!(output.contains("Updated Article 1"));
    }

    #[test]
    fn test_no_changes() {
        let driver = FeedDriver::new();
        let output = driver.format_diff(Some(RSS_BASE), RSS_BASE).unwrap();
        assert_eq!(output, "no changes");
    }
}
