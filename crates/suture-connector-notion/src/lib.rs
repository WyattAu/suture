//! Notion API connector for Suture.
//!
//! Syncs Notion pages and databases into local markdown and JSON files
//! that Suture can track and merge.
//!
//! # Usage
//!
//! ```rust,ignore
//! use suture_connector_notion::NotionClient;
//!
//! let client = NotionClient::new("secret_notion_integration_token");
//! let pages = client.list_pages(None).await?;
//! let markdown = client.page_to_markdown(&page).await?;
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from Notion API operations.
#[derive(Debug, Error)]
pub enum NotionError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Notion API error: {status} — {message}")]
    Api { status: u16, message: String },
    #[error("response parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("page content is empty")]
    EmptyContent,
    #[error("block type not supported: {0}")]
    UnsupportedBlock(String),
    #[error("rate limited — retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
}

// ---------------------------------------------------------------------------
// Notion API types
// ---------------------------------------------------------------------------

/// A Notion page object (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionPage {
    pub id: String,
    #[serde(default)]
    pub created_time: String,
    #[serde(default)]
    pub last_edited_time: String,
    #[serde(default)]
    pub archived: bool,
    pub properties: serde_json::Value,
    #[serde(default)]
    pub url: String,
}

/// A Notion block (rich text, heading, list, code, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionBlock {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub paragraph: Option<BlockContent>,
    #[serde(default)]
    pub heading_1: Option<BlockContent>,
    #[serde(default)]
    pub heading_2: Option<BlockContent>,
    #[serde(default)]
    pub heading_3: Option<BlockContent>,
    #[serde(default)]
    pub bulleted_list_item: Option<BlockContent>,
    #[serde(default)]
    pub numbered_list_item: Option<BlockContent>,
    #[serde(default)]
    pub code: Option<CodeBlockContent>,
    #[serde(default)]
    pub quote: Option<BlockContent>,
    #[serde(default)]
    pub callout: Option<CalloutContent>,
    #[serde(default)]
    pub divider: Option<serde_json::Value>,
    #[serde(default)]
    pub table: Option<TableContent>,
    #[serde(default)]
    pub child_database: Option<ChildDatabaseContent>,
    #[serde(default)]
    pub bookmark: Option<BookmarkContent>,
    #[serde(default)]
    pub image: Option<FileContent>,
    #[serde(default)]
    pub has_children: bool,
}

/// Rich text content within a block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockContent {
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub color: String,
}

/// A rich text element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichText {
    #[serde(rename = "type")]
    pub text_type: String,
    #[serde(default)]
    pub text: Option<TextContent>,
    #[serde(default)]
    pub equation: Option<serde_json::Value>,
    #[serde(default)]
    pub plain_text: String,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub annotations: Annotations,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    pub content: String,
    #[serde(default)]
    pub link: Option<Link>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotations {
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub strikethrough: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub code: bool,
    #[serde(default)]
    pub color: String,
}

impl Default for Annotations {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            strikethrough: false,
            underline: false,
            code: false,
            color: "default".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlockContent {
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalloutContent {
    pub rich_text: Vec<RichText>,
    #[serde(default)]
    pub icon: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableContent {
    #[serde(default)]
    pub table_width: i32,
    #[serde(default)]
    pub has_column_header: bool,
    #[serde(default)]
    pub has_row_header: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildDatabaseContent {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkContent {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    #[serde(default)]
    pub file: Option<FileInfo>,
    #[serde(default)]
    pub external: Option<FileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub url: String,
}

/// Notion API list response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ListResponse<T> {
    results: Vec<T>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_cursor: Option<String>,
}

/// Notion API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotionApiResponse {
    #[serde(default)]
    message: String,
}

/// A database row from Notion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseRow {
    pub id: String,
    pub properties: serde_json::Value,
    #[serde(default)]
    pub created_time: String,
    #[serde(default)]
    pub last_edited_time: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the Notion API.
pub struct NotionClient {
    http: reqwest::Client,
    token: String,
    base_url: String,
}

impl NotionClient {
    const NOTION_VERSION: &str = "2022-06-28";

    /// Create a new Notion API client.
    ///
    /// `token` should be an integration token starting with `ntn_` or `secret_`.
    pub fn new(token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.to_owned(),
            base_url: "https://api.notion.com/v1".to_owned(),
        }
    }

    /// Create a client with a custom base URL (for testing).
    #[cfg(test)]
    pub fn with_base_url(token: &str, base_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.to_owned(),
            base_url: base_url.to_owned(),
        }
    }

    /// List all pages accessible to the integration.
    pub async fn list_pages(
        &self,
        start_cursor: Option<&str>,
    ) -> Result<(Vec<NotionPage>, Option<String>), NotionError> {
        let mut url = format!("{}/pages", self.base_url);
        if let Some(cursor) = start_cursor {
            url.push_str(&format!("?start_cursor={cursor}"));
        }

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", Self::NOTION_VERSION)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status == 429 {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1000);
            return Err(NotionError::RateLimited {
                retry_after_ms: retry,
            });
        }
        if !resp.status().is_success() {
            let body: NotionApiResponse = resp.json().await.unwrap_or(NotionApiResponse {
                message: format!("HTTP {status}"),
            });
            return Err(NotionError::Api {
                status,
                message: body.message,
            });
        }

        let data: ListResponse<NotionPage> = resp.json().await?;
        Ok((data.results, data.next_cursor))
    }

    /// List all pages, handling pagination automatically.
    pub async fn list_all_pages(&self) -> Result<Vec<NotionPage>, NotionError> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let (pages, next) = self.list_pages(cursor.as_deref()).await?;
            all.extend(pages);
            match next {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        Ok(all)
    }

    /// Get the blocks (content) of a page.
    pub async fn get_page_blocks(&self, page_id: &str) -> Result<Vec<NotionBlock>, NotionError> {
        let url = format!("{}/blocks/{page_id}/children?page_size=100", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", Self::NOTION_VERSION)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status == 429 {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1000);
            return Err(NotionError::RateLimited {
                retry_after_ms: retry,
            });
        }
        if !resp.status().is_success() {
            let body: NotionApiResponse = resp.json().await.unwrap_or(NotionApiResponse {
                message: format!("HTTP {status}"),
            });
            return Err(NotionError::Api {
                status,
                message: body.message,
            });
        }

        let data: ListResponse<NotionBlock> = resp.json().await?;
        Ok(data.results)
    }

    /// Query a database.
    pub async fn query_database(&self, database_id: &str) -> Result<Vec<DatabaseRow>, NotionError> {
        let url = format!("{}/databases/{database_id}/query", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", Self::NOTION_VERSION)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({}))
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status == 429 {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1000);
            return Err(NotionError::RateLimited {
                retry_after_ms: retry,
            });
        }
        if !resp.status().is_success() {
            let body: NotionApiResponse = resp.json().await.unwrap_or(NotionApiResponse {
                message: format!("HTTP {status}"),
            });
            return Err(NotionError::Api {
                status,
                message: body.message,
            });
        }

        let data: ListResponse<DatabaseRow> = resp.json().await?;
        Ok(data.results)
    }

    /// Convert a page's blocks to markdown.
    pub async fn page_to_markdown(&self, page_id: &str) -> Result<String, NotionError> {
        let blocks = self.get_page_blocks(page_id).await?;
        if blocks.is_empty() {
            return Err(NotionError::EmptyContent);
        }
        let md = blocks_to_markdown(&blocks);
        Ok(md)
    }

    /// Convert a page's blocks to markdown, given pre-fetched blocks.
    pub fn blocks_to_markdown(blocks: &[NotionBlock]) -> Result<String, NotionError> {
        if blocks.is_empty() {
            return Err(NotionError::EmptyContent);
        }
        Ok(blocks_to_markdown(blocks))
    }

    /// Convert a database to JSON array of rows.
    pub async fn database_to_json(&self, database_id: &str) -> Result<String, NotionError> {
        let rows = self.query_database(database_id).await?;
        let json = serde_json::to_string_pretty(&rows)?;
        Ok(json)
    }
}

// ---------------------------------------------------------------------------
// Markdown conversion
// ---------------------------------------------------------------------------

fn rich_text_to_markdown(texts: &[RichText]) -> String {
    let mut result = String::new();
    for rt in texts {
        let content = match &rt.text {
            Some(t) => &t.content,
            None => &rt.plain_text,
        };
        let annotations = &rt.annotations;
        let mut formatted = String::new();
        if annotations.code {
            formatted.push('`');
        }
        if annotations.bold {
            formatted.push_str("**");
        }
        if annotations.italic {
            formatted.push('*');
        }
        if annotations.strikethrough {
            formatted.push_str("~~");
        }
        formatted.push_str(content);
        if annotations.strikethrough {
            formatted.push_str("~~");
        }
        if annotations.italic {
            formatted.push('*');
        }
        if annotations.bold {
            formatted.push_str("**");
        }
        if annotations.code {
            formatted.push('`');
        }
        if let Some(href) = &rt.href {
            result.push_str(&format!("[{formatted}]({href})"));
        } else {
            result.push_str(&formatted);
        }
    }
    result
}

/// Extract the content field from a block, regardless of block type.
/// Useful for consumers that need raw access to block content.
#[must_use]
pub fn extract_content(block: &NotionBlock) -> Option<&BlockContent> {
    match block.block_type.as_str() {
        "paragraph" => block.paragraph.as_ref(),
        "heading_1" => block.heading_1.as_ref(),
        "heading_2" => block.heading_2.as_ref(),
        "heading_3" => block.heading_3.as_ref(),
        "bulleted_list_item" => block.bulleted_list_item.as_ref(),
        "numbered_list_item" => block.numbered_list_item.as_ref(),
        "quote" => block.quote.as_ref(),
        _ => None,
    }
}

fn blocks_to_markdown(blocks: &[NotionBlock]) -> String {
    let mut md = String::new();
    let mut numbered_counter = 0u32;

    for block in blocks {
        match block.block_type.as_str() {
            "paragraph" => {
                if let Some(content) = &block.paragraph {
                    let text = rich_text_to_markdown(&content.rich_text);
                    if !text.is_empty() {
                        md.push_str(&text);
                        md.push('\n');
                    }
                    md.push('\n');
                }
            }
            "heading_1" => {
                if let Some(content) = &block.heading_1 {
                    md.push_str("# ");
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push_str("\n\n");
                }
            }
            "heading_2" => {
                if let Some(content) = &block.heading_2 {
                    md.push_str("## ");
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push_str("\n\n");
                }
            }
            "heading_3" => {
                if let Some(content) = &block.heading_3 {
                    md.push_str("### ");
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push_str("\n\n");
                }
            }
            "bulleted_list_item" => {
                if let Some(content) = &block.bulleted_list_item {
                    md.push_str("- ");
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push('\n');
                }
            }
            "numbered_list_item" => {
                numbered_counter += 1;
                if let Some(content) = &block.numbered_list_item {
                    md.push_str(&format!("{}. ", numbered_counter));
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push('\n');
                }
            }
            "code" => {
                if let Some(content) = &block.code {
                    let lang = if content.language.is_empty() {
                        String::new()
                    } else {
                        content.language.clone()
                    };
                    md.push_str(&format!("```{lang}\n"));
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push_str("\n```\n\n");
                }
            }
            "quote" => {
                if let Some(content) = &block.quote {
                    md.push_str("> ");
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push_str("\n\n");
                }
            }
            "callout" => {
                if let Some(content) = &block.callout {
                    md.push_str("> ");
                    md.push_str(&rich_text_to_markdown(&content.rich_text));
                    md.push_str("\n\n");
                }
            }
            "divider" => {
                md.push_str("---\n\n");
            }
            "table" => {
                md.push_str("<!-- table: convert manually in Notion -->\n\n");
            }
            "child_database" => {
                if let Some(content) = &block.child_database {
                    md.push_str(&format!("[Database: {}]\n\n", content.title));
                }
            }
            "bookmark" => {
                if let Some(content) = &block.bookmark {
                    md.push_str(&format!("[Bookmark]({})\n\n", content.url));
                }
            }
            "image" => {
                let url = block
                    .image
                    .as_ref()
                    .and_then(|f| f.file.as_ref())
                    .map(|f| &f.url)
                    .or_else(|| {
                        block
                            .image
                            .as_ref()
                            .and_then(|f| f.external.as_ref())
                            .map(|f| &f.url)
                    });
                if let Some(url) = url {
                    md.push_str(&format!("![image]({url})\n\n"));
                }
            }
            _ => {
                md.push_str(&format!(
                    "<!-- unsupported block type: {} -->\n\n",
                    block.block_type
                ));
            }
        }

        // Reset numbered list counter on non-list blocks
        if !matches!(
            block.block_type.as_str(),
            "numbered_list_item" | "bulleted_list_item"
        ) {
            numbered_counter = 0;
        }
    }

    md.trim_end().to_owned() + "\n"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rich_text(content: &str) -> Vec<RichText> {
        vec![RichText {
            text_type: "text".to_owned(),
            text: Some(TextContent {
                content: content.to_owned(),
                link: None,
            }),
            equation: None,
            plain_text: content.to_owned(),
            href: None,
            annotations: Annotations::default(),
        }]
    }

    fn make_bold_text(content: &str) -> Vec<RichText> {
        vec![RichText {
            text_type: "text".to_owned(),
            text: Some(TextContent {
                content: content.to_owned(),
                link: None,
            }),
            equation: None,
            plain_text: content.to_owned(),
            href: None,
            annotations: Annotations {
                bold: true,
                ..Annotations::default()
            },
        }]
    }

    fn make_link_text(content: &str, url: &str) -> Vec<RichText> {
        vec![RichText {
            text_type: "text".to_owned(),
            text: Some(TextContent {
                content: content.to_owned(),
                link: Some(Link {
                    url: url.to_owned(),
                }),
            }),
            equation: None,
            plain_text: content.to_owned(),
            href: Some(url.to_owned()),
            annotations: Annotations::default(),
        }]
    }

    fn make_block(block_type: &str, rich_text: Vec<RichText>) -> NotionBlock {
        let content = BlockContent {
            rich_text,
            color: "default".to_owned(),
        };
        NotionBlock {
            id: "test-id".to_owned(),
            block_type: block_type.to_owned(),
            paragraph: if block_type == "paragraph" {
                Some(content.clone())
            } else {
                None
            },
            heading_1: if block_type == "heading_1" {
                Some(content.clone())
            } else {
                None
            },
            heading_2: if block_type == "heading_2" {
                Some(content.clone())
            } else {
                None
            },
            heading_3: if block_type == "heading_3" {
                Some(content.clone())
            } else {
                None
            },
            bulleted_list_item: if block_type == "bulleted_list_item" {
                Some(content.clone())
            } else {
                None
            },
            numbered_list_item: if block_type == "numbered_list_item" {
                Some(content.clone())
            } else {
                None
            },
            code: if block_type == "code" {
                Some(CodeBlockContent {
                    rich_text: content.rich_text.clone(),
                    language: "rust".to_owned(),
                })
            } else {
                None
            },
            quote: if block_type == "quote" {
                Some(content.clone())
            } else {
                None
            },
            callout: None,
            divider: if block_type == "divider" {
                Some(serde_json::json!({}))
            } else {
                None
            },
            table: None,
            child_database: None,
            bookmark: None,
            image: None,
            has_children: false,
        }
    }

    #[test]
    fn test_rich_text_to_plain() {
        let texts = make_rich_text("hello world");
        assert_eq!(rich_text_to_markdown(&texts), "hello world");
    }

    #[test]
    fn test_rich_text_bold() {
        let texts = make_bold_text("bold text");
        assert_eq!(rich_text_to_markdown(&texts), "**bold text**");
    }

    #[test]
    fn test_rich_text_link() {
        let texts = make_link_text("click here", "https://example.com");
        assert_eq!(
            rich_text_to_markdown(&texts),
            "[click here](https://example.com)"
        );
    }

    #[test]
    fn test_paragraph_to_markdown() {
        let blocks = vec![make_block("paragraph", make_rich_text("Hello, world!"))];
        let md = blocks_to_markdown(&blocks);
        assert_eq!(md, "Hello, world!\n");
    }

    #[test]
    fn test_heading_to_markdown() {
        let blocks = vec![
            make_block("heading_1", make_rich_text("Title")),
            make_block("heading_2", make_rich_text("Subtitle")),
            make_block("heading_3", make_rich_text("Section")),
        ];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("# Title"));
        assert!(md.contains("## Subtitle"));
        assert!(md.contains("### Section"));
    }

    #[test]
    fn test_list_to_markdown() {
        let blocks = vec![
            make_block("bulleted_list_item", make_rich_text("item 1")),
            make_block("bulleted_list_item", make_rich_text("item 2")),
        ];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("- item 1"));
        assert!(md.contains("- item 2"));
    }

    #[test]
    fn test_numbered_list_to_markdown() {
        let blocks = vec![
            make_block("numbered_list_item", make_rich_text("first")),
            make_block("numbered_list_item", make_rich_text("second")),
        ];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("1. first"));
        assert!(md.contains("2. second"));
    }

    #[test]
    fn test_code_block_to_markdown() {
        let blocks = vec![make_block("code", make_rich_text("fn main() {}"))];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main() {}"));
        assert!(md.contains("```"));
    }

    #[test]
    fn test_divider_to_markdown() {
        let blocks = vec![make_block("divider", vec![])];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("---"));
    }

    #[test]
    fn test_mixed_blocks_to_markdown() {
        let blocks = vec![
            make_block("heading_1", make_rich_text("Doc")),
            make_block("paragraph", make_rich_text("Intro text.")),
            make_block("heading_2", make_rich_text("Details")),
            make_block("bulleted_list_item", make_rich_text("point a")),
            make_block("bulleted_list_item", make_rich_text("point b")),
            make_block("code", make_rich_text("x = 42")),
            make_block("divider", vec![]),
        ];
        let md = blocks_to_markdown(&blocks);
        assert!(md.starts_with("# Doc"));
        assert!(md.contains("Intro text."));
        assert!(md.contains("## Details"));
        assert!(md.contains("- point a"));
        assert!(md.contains("- point b"));
        assert!(md.contains("```rust"));
        assert!(md.contains("---"));
    }

    #[test]
    fn test_empty_blocks_error() {
        let result = NotionClient::blocks_to_markdown(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_annotations_default() {
        let a = Annotations::default();
        assert!(!a.bold);
        assert!(!a.italic);
        assert!(!a.code);
    }
}
