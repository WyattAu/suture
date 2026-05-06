//! Airtable API connector for Suture.
//!
//! Syncs Airtable bases/tables into local JSON and CSV files
//! that Suture can track and merge.
//!
//! # Usage
//!
//! ```rust,ignore
//! use suture_connector_airtable::AirtableClient;
//!
//! let client = AirtableClient::new("patXXX...your_token", "appXXX...your_base_id");
//! let records = client.list_records("Table1", None).await?;
//! let json = client.table_to_json("Table1").await?;
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from Airtable API operations.
#[derive(Debug, Error)]
pub enum AirtableError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Airtable API error: {status} — {message}")]
    Api { status: u16, message: String },
    #[error("response parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("table is empty")]
    EmptyTable,
    #[error("rate limited — retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
}

// ---------------------------------------------------------------------------
// Airtable API types
// ---------------------------------------------------------------------------

/// An Airtable record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirtableRecord {
    pub id: String,
    #[serde(default)]
    pub created_time: String,
    pub fields: serde_json::Value,
}

/// A field schema from Airtable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
}

/// A table schema from Airtable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub fields: Vec<FieldSchema>,
}

/// Base schema (all tables).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseSchema {
    pub tables: Vec<TableSchema>,
}

/// List records response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ListRecordsResponse {
    records: Vec<AirtableRecord>,
    #[serde(default)]
    offset: Option<String>,
}

/// API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AirtableErrorResponse {
    #[serde(default)]
    error: Option<AirtableErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AirtableErrorBody {
    #[serde(default)]
    message: String,
    #[serde(default)]
    r#type: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the Airtable REST API.
pub struct AirtableClient {
    http: reqwest::Client,
    token: String,
    base_id: String,
    base_url: String,
}

impl AirtableClient {
    /// Create a new Airtable API client.
    ///
    /// `token` is a personal access token starting with `pat`.
    /// `base_id` is the base ID starting with `app`.
    pub fn new(token: &str, base_id: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.to_owned(),
            base_id: base_id.to_owned(),
            base_url: "https://api.airtable.com/v0".to_owned(),
        }
    }

    /// Create a client with custom base URL (for testing).
    #[cfg(test)]
    pub fn with_base_url(token: &str, base_id: &str, base_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.to_owned(),
            base_id: base_id.to_owned(),
            base_url: base_url.to_owned(),
        }
    }

    /// List records from a table.
    pub async fn list_records(
        &self,
        table: &str,
        offset: Option<&str>,
    ) -> Result<(Vec<AirtableRecord>, Option<String>), AirtableError> {
        let mut url = format!("{}/{}/{}", self.base_url, self.base_id, table);
        if let Some(off) = offset {
            url.push_str(&format!("?offset={off}"));
        }

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
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
            return Err(AirtableError::RateLimited {
                retry_after_ms: retry,
            });
        }
        if !resp.status().is_success() {
            let body: AirtableErrorResponse = resp
                .json()
                .await
                .unwrap_or(AirtableErrorResponse { error: None });
            let message = body
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(AirtableError::Api { status, message });
        }

        let data: ListRecordsResponse = resp.json().await?;
        Ok((data.records, data.offset))
    }

    /// List all records from a table, handling pagination.
    pub async fn list_all_records(
        &self,
        table: &str,
    ) -> Result<Vec<AirtableRecord>, AirtableError> {
        let mut all = Vec::new();
        let mut offset: Option<String> = None;

        loop {
            let (records, next) = self.list_records(table, offset.as_deref()).await?;
            all.extend(records);
            match next {
                Some(o) => offset = Some(o),
                None => break,
            }
        }

        Ok(all)
    }

    /// Get the base schema (all tables and their fields).
    pub async fn get_schema(&self) -> Result<BaseSchema, AirtableError> {
        let url = format!("{}/meta/bases/{}/tables", self.base_url, self.base_id);

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body: AirtableErrorResponse = resp
                .json()
                .await
                .unwrap_or(AirtableErrorResponse { error: None });
            let message = body
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(AirtableError::Api { status, message });
        }

        let schema: BaseSchema = resp.json().await?;
        Ok(schema)
    }

    /// Convert a table to JSON (array of record objects).
    pub async fn table_to_json(&self, table: &str) -> Result<String, AirtableError> {
        let records = self.list_all_records(table).await?;
        if records.is_empty() {
            return Err(AirtableError::EmptyTable);
        }
        // Extract just the fields for a clean output
        let fields: Vec<&serde_json::Value> = records.iter().map(|r| &r.fields).collect();
        let json = serde_json::to_string_pretty(&fields)?;
        Ok(json)
    }

    /// Convert a table to CSV string.
    ///
    /// Uses the first record to determine column headers.
    pub async fn table_to_csv(&self, table: &str) -> Result<String, AirtableError> {
        let records = self.list_all_records(table).await?;
        if records.is_empty() {
            return Err(AirtableError::EmptyTable);
        }
        Ok(records_to_csv(&records))
    }
}

// ---------------------------------------------------------------------------
// CSV conversion
// ---------------------------------------------------------------------------

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_owned()
    }
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Array(arr) => {
            // Join array elements with semicolons
            let parts: Vec<String> = arr.iter().map(json_value_to_string).collect();
            parts.join("; ")
        }
        serde_json::Value::Object(_) => {
            // Serialize nested objects as JSON
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned())
        }
    }
}

fn records_to_csv(records: &[AirtableRecord]) -> String {
    // Collect all field names in order of first appearance
    let mut headers: Vec<String> = Vec::new();
    let mut header_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    for record in records {
        if let serde_json::Value::Object(map) = &record.fields {
            for key in map.keys() {
                if header_set.insert(key.clone()) {
                    headers.push(key.clone());
                }
            }
        }
    }

    let mut csv = String::new();

    // Header row
    let header_line: Vec<String> = headers.iter().map(|h| csv_escape(h)).collect();
    csv.push_str(&header_line.join(","));
    csv.push('\n');

    // Data rows
    for record in records {
        let mut row = Vec::new();
        for header in &headers {
            let value = record
                .fields
                .get(header)
                .map(json_value_to_string)
                .unwrap_or_default();
            row.push(csv_escape(&value));
        }
        csv.push_str(&row.join(","));
        csv.push('\n');
    }

    csv
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(fields: Vec<(&str, serde_json::Value)>) -> AirtableRecord {
        let mut map = serde_json::Map::new();
        for (k, v) in fields {
            map.insert(k.to_owned(), v);
        }
        AirtableRecord {
            id: "recXXX".to_owned(),
            created_time: "2024-01-01T00:00:00.000Z".to_owned(),
            fields: serde_json::Value::Object(map),
        }
    }

    #[test]
    fn test_csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn test_csv_escape_comma() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    #[test]
    fn test_csv_escape_quote() {
        // RFC 4180: quotes are escaped by doubling them
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_csv_escape_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_json_value_string() {
        assert_eq!(json_value_to_string(&serde_json::json!("hello")), "hello");
    }

    #[test]
    fn test_json_value_number() {
        assert_eq!(json_value_to_string(&serde_json::json!(42)), "42");
    }

    #[test]
    fn test_json_value_bool() {
        assert_eq!(json_value_to_string(&serde_json::json!(true)), "true");
    }

    #[test]
    fn test_json_value_null() {
        assert_eq!(json_value_to_string(&serde_json::Value::Null), "");
    }

    #[test]
    fn test_json_value_array() {
        let arr = serde_json::json!(["a", "b", "c"]);
        assert_eq!(json_value_to_string(&arr), "a; b; c");
    }

    #[test]
    fn test_json_value_object() {
        let obj = serde_json::json!({"key": "val"});
        let result = json_value_to_string(&obj);
        assert!(result.contains("\"key\""));
        assert!(result.contains("\"val\""));
    }

    #[test]
    fn test_records_to_csv_simple() {
        let records = vec![
            make_record(vec![
                ("Name", serde_json::json!("Alice")),
                ("Age", serde_json::json!(30)),
            ]),
            make_record(vec![
                ("Name", serde_json::json!("Bob")),
                ("Age", serde_json::json!(25)),
            ]),
        ];
        let csv = records_to_csv(&records);
        let lines: Vec<&str> = csv.trim_end().lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("Name"));
        assert!(lines[0].contains("Age"));
        assert!(lines[1].contains("Alice"));
        assert!(lines[1].contains("30"));
        assert!(lines[2].contains("Bob"));
        assert!(lines[2].contains("25"));
    }

    #[test]
    fn test_records_to_csv_comma_value() {
        let records = vec![make_record(vec![(
            "Notes",
            serde_json::json!("has, commas"),
        )])];
        let csv = records_to_csv(&records);
        assert!(csv.contains("\"has, commas\""));
    }

    #[test]
    fn test_records_to_csv_missing_fields() {
        let records = vec![
            make_record(vec![
                ("A", serde_json::json!(1)),
                ("B", serde_json::json!(2)),
            ]),
            make_record(vec![("A", serde_json::json!(3))]), // Missing B
        ];
        let csv = records_to_csv(&records);
        let lines: Vec<&str> = csv.trim_end().lines().collect();
        assert_eq!(lines.len(), 3);
        // Second data row should have empty B column
        let parts: Vec<&str> = lines[2].split(',').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "3");
        assert_eq!(parts[1], "");
    }

    #[test]
    fn test_empty_records_error() {
        let result = records_to_csv(&[]);
        let lines: Vec<&str> = result.lines().collect();
        // Empty records still produces just headers (which are empty)
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_empty());
    }

    #[test]
    fn test_records_to_csv_nested_array() {
        let records = vec![make_record(vec![(
            "Tags",
            serde_json::json!(["rust", "wasm"]),
        )])];
        let csv = records_to_csv(&records);
        assert!(csv.contains("rust; wasm"));
    }
}
