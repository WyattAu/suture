//! Google Sheets API connector for Suture.
//!
//! Syncs Google Sheets spreadsheets into local CSV and JSON files
//! that Suture can track and merge.
//!
//! # Authentication
//!
//! Google Sheets API uses OAuth 2.0 service account or user credentials.
//! Pass a valid access token (Bearer) to the client constructor.
//!
//! # Usage
//!
//! ```rust,ignore
//! use suture_connector_gsheets::SheetsClient;
//!
//! let client = SheetsClient::new("your-access-token");
//! let values = client.get_values("spreadsheet-id", "Sheet1!A1:Z1000").await?;
//! let csv = client.sheet_to_csv(&values)?;
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from Google Sheets API operations.
#[derive(Debug, Error)]
pub enum SheetsError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Google Sheets API error: {status} — {message}")]
    Api { status: u16, message: String },
    #[error("response parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("no data in range")]
    EmptyData,
    #[error("rate limited — retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("invalid range: {0}")]
    InvalidRange(String),
}

// ---------------------------------------------------------------------------
// Google Sheets API types
// ---------------------------------------------------------------------------

/// A single cell value in a sheet row.
pub type RowValues = Vec<String>;

/// A grid of cell values from a sheet range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueRange {
    /// The range that was read (e.g., "Sheet1!A1:Z100").
    #[serde(default)]
    pub range: String,
    /// The major dimension: "ROWS" or "COLUMNS".
    #[serde(default = "default_major_dimension")]
    pub major_dimension: String,
    /// The actual cell values.
    #[serde(default)]
    pub values: Vec<Vec<String>>,
}

fn default_major_dimension() -> String {
    "ROWS".to_owned()
}

/// Spreadsheet metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Spreadsheet {
    #[serde(rename = "spreadsheetId")]
    pub spreadsheet_id: String,
    #[serde(default)]
    pub properties: SpreadsheetProperties,
    #[serde(default)]
    pub sheets: Vec<Sheet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpreadsheetProperties {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sheet {
    #[serde(default)]
    pub properties: SheetProperties,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SheetProperties {
    #[serde(default, rename = "sheetId")]
    pub sheet_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub index: i64,
    #[serde(default, rename = "sheetType")]
    pub sheet_type: String,
    #[serde(default, rename = "gridProperties")]
    pub grid_properties: GridProperties,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GridProperties {
    #[serde(default, rename = "rowCount")]
    pub row_count: i64,
    #[serde(default, rename = "columnCount")]
    pub column_count: i64,
}

/// API error response from Google.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiErrorResponse {
    #[serde(default)]
    error: Option<ApiErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiErrorDetail {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
    #[serde(default)]
    status: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the Google Sheets API v4.
pub struct SheetsClient {
    http: reqwest::Client,
    token: String,
    base_url: String,
}

impl SheetsClient {
    const API_VERSION: &str = "v4";

    /// Create a new Google Sheets API client.
    ///
    /// `token` should be a valid OAuth 2.0 access token.
    pub fn new(token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.to_owned(),
            base_url: format!("https://sheets.googleapis.com/{}", Self::API_VERSION),
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

    /// Get spreadsheet metadata.
    pub async fn get_spreadsheet(
        &self,
        spreadsheet_id: &str,
    ) -> Result<Spreadsheet, SheetsError> {
        let url = format!(
            "{}/spreadsheets/{}?includeGridData=false",
            self.base_url, spreadsheet_id
        );
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
            return Err(SheetsError::RateLimited { retry_after_ms: retry });
        }
        if !resp.status().is_success() {
            let body: ApiErrorResponse = resp.json().await.unwrap_or(ApiErrorResponse {
                error: None,
            });
            let message = body
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(SheetsError::Api { status, message });
        }

        resp.json::<Spreadsheet>().await.map_err(|e| SheetsError::Api {
            status: 0,
            message: format!("failed to parse spreadsheet metadata: {e}"),
        })
    }

    /// Get values from a sheet range.
    ///
    /// `range` should be in A1 notation (e.g., "Sheet1!A1:Z100").
    pub async fn get_values(
        &self,
        spreadsheet_id: &str,
        range: &str,
    ) -> Result<ValueRange, SheetsError> {
        let url = format!(
            "{}/spreadsheets/{}/values/{}",
            self.base_url, spreadsheet_id, urlencoding::encode(range)
        );
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
            return Err(SheetsError::RateLimited { retry_after_ms: retry });
        }
        if !resp.status().is_success() {
            let body: ApiErrorResponse = resp.json().await.unwrap_or(ApiErrorResponse {
                error: None,
            });
            let message = body
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(SheetsError::Api { status, message });
        }

        resp.json::<ValueRange>().await.map_err(|e| SheetsError::Api {
            status: 0,
            message: format!("failed to parse value range: {e}"),
        })
    }

    /// Get all sheet names from a spreadsheet.
    pub async fn list_sheet_names(
        &self,
        spreadsheet_id: &str,
    ) -> Result<Vec<String>, SheetsError> {
        let meta = self.get_spreadsheet(spreadsheet_id).await?;
        let names: Vec<String> = meta
            .sheets
            .into_iter()
            .map(|s| s.properties.title)
            .collect();
        Ok(names)
    }

    /// Get values from an entire sheet (all rows and columns).
    ///
    /// Uses the sheet's grid dimensions to determine the range.
    pub async fn get_full_sheet(
        &self,
        spreadsheet_id: &str,
        sheet_name: &str,
    ) -> Result<ValueRange, SheetsError> {
        // First get metadata to find grid dimensions.
        let meta = self.get_spreadsheet(spreadsheet_id).await?;
        let sheet_meta = meta
            .sheets
            .iter()
            .find(|s| s.properties.title == sheet_name)
            .ok_or_else(|| SheetsError::InvalidRange(format!("sheet '{sheet_name}' not found")))?;

        let rows = sheet_meta.properties.grid_properties.row_count;
        let cols = sheet_meta.properties.grid_properties.column_count;

        // Convert column count to letter (A, B, ..., Z, AA, AB, ...).
        let col_letter = column_number_to_letter(cols);
        let range = format!("{sheet_name}!A1:{col_letter}{rows}");

        self.get_values(spreadsheet_id, &range).await
    }

    /// Convert a `ValueRange` to CSV format.
    pub fn values_to_csv(values: &ValueRange) -> Result<String, SheetsError> {
        if values.values.is_empty() {
            return Err(SheetsError::EmptyData);
        }

        let mut csv = String::new();
        for row in &values.values {
            let escaped: Vec<String> = row.iter().map(|cell| csv_escape(cell)).collect();
            csv.push_str(&escaped.join(","));
            csv.push('\n');
        }

        Ok(csv)
    }

    /// Convert a `ValueRange` to a JSON array of objects.
    ///
    /// The first row is used as headers. Each subsequent row becomes
    /// a JSON object with header keys.
    pub fn values_to_json_objects(values: &ValueRange) -> Result<String, SheetsError> {
        if values.values.is_empty() {
            return Err(SheetsError::EmptyData);
        }

        let headers = &values.values[0];
        if headers.is_empty() {
            return Err(SheetsError::EmptyData);
        }

        let mut objects = Vec::new();
        for row in &values.values[1..] {
            let mut obj = serde_json::Map::new();
            for (i, header) in headers.iter().enumerate() {
                let value = row.get(i).cloned().unwrap_or_default();
                // Try to parse numbers and booleans from string representations.
                let json_value = if value.is_empty() {
                    serde_json::Value::Null
                } else if let Ok(num) = value.parse::<i64>() {
                    serde_json::json!(num)
                } else if let Ok(num) = value.parse::<f64>() {
                    serde_json::json!(num)
                } else if value == "TRUE" || value == "true" {
                    serde_json::json!(true)
                } else if value == "FALSE" || value == "false" {
                    serde_json::json!(false)
                } else {
                    serde_json::json!(value)
                };
                obj.insert(header.clone(), json_value);
            }
            objects.push(serde_json::Value::Object(obj));
        }

        serde_json::to_string_pretty(&objects).map_err(SheetsError::Parse)
    }

    /// Convert a `ValueRange` to a JSON array of arrays (preserving structure).
    pub fn values_to_json_arrays(values: &ValueRange) -> Result<String, SheetsError> {
        if values.values.is_empty() {
            return Err(SheetsError::EmptyData);
        }
        serde_json::to_string_pretty(&values.values).map_err(SheetsError::Parse)
    }

    /// Get a sheet as CSV.
    pub async fn sheet_to_csv(
        &self,
        spreadsheet_id: &str,
        range: &str,
    ) -> Result<String, SheetsError> {
        let values = self.get_values(spreadsheet_id, range).await?;
        Self::values_to_csv(&values)
    }

    /// Get a sheet as JSON objects (header → object per row).
    pub async fn sheet_to_json(
        &self,
        spreadsheet_id: &str,
        range: &str,
    ) -> Result<String, SheetsError> {
        let values = self.get_values(spreadsheet_id, range).await?;
        Self::values_to_json_objects(&values)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a column number (1-based) to an Excel-style letter (A, B, ..., Z, AA, ...).
fn column_number_to_letter(n: i64) -> String {
    let mut n = n;
    let mut result = String::new();
    while n > 0 {
        n -= 1;
        let remainder = (n % 26) as u8;
        result.insert(0, (b'A' + remainder) as char);
        n /= 26;
    }
    if result.is_empty() {
        result.push('A');
    }
    result
}

/// Escape a cell value for CSV output per RFC 4180.
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r')
    {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_owned()
    }
}

/// Minimal URL encoding for API parameters.
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for byte in s.as_bytes() {
            match *byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'!' | b'\''
                | b'(' | b')' | b'*' => {
                    result.push(*byte as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_number_to_letter() {
        assert_eq!(column_number_to_letter(1), "A");
        assert_eq!(column_number_to_letter(2), "B");
        assert_eq!(column_number_to_letter(26), "Z");
        assert_eq!(column_number_to_letter(27), "AA");
        assert_eq!(column_number_to_letter(28), "AB");
        assert_eq!(column_number_to_letter(52), "AZ");
        assert_eq!(column_number_to_letter(53), "BA");
        assert_eq!(column_number_to_letter(702), "ZZ");
        assert_eq!(column_number_to_letter(703), "AAA");
    }

    #[test]
    fn test_csv_escape_simple() {
        assert_eq!(csv_escape("hello"), "hello");
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
        assert_eq!(csv_escape(""), "");
    }

    #[test]
    fn test_values_to_csv() {
        let values = ValueRange {
            range: "Sheet1!A1:B2".to_owned(),
            major_dimension: "ROWS".to_owned(),
            values: vec![
                vec!["Name".to_owned(), "Age".to_owned()],
                vec!["Alice".to_owned(), "30".to_owned()],
                vec!["Bob".to_owned(), "25".to_owned()],
            ],
        };
        let csv = SheetsClient::values_to_csv(&values).unwrap();
        assert_eq!(csv, "Name,Age\nAlice,30\nBob,25\n");
    }

    #[test]
    fn test_values_to_csv_with_commas() {
        let values = ValueRange {
            range: "Sheet1!A1:B2".to_owned(),
            major_dimension: "ROWS".to_owned(),
            values: vec![
                vec!["Name".to_owned(), "Description".to_owned()],
                vec!["Item".to_owned(), "Has a, comma".to_owned()],
            ],
        };
        let csv = SheetsClient::values_to_csv(&values).unwrap();
        assert_eq!(csv, "Name,Description\nItem,\"Has a, comma\"\n");
    }

    #[test]
    fn test_values_to_csv_empty() {
        let values = ValueRange {
            range: "Sheet1!A1".to_owned(),
            major_dimension: "ROWS".to_owned(),
            values: vec![],
        };
        assert!(SheetsClient::values_to_csv(&values).is_err());
    }

    #[test]
    fn test_values_to_json_objects() {
        let values = ValueRange {
            range: "Sheet1!A1:C3".to_owned(),
            major_dimension: "ROWS".to_owned(),
            values: vec![
                vec!["name".to_owned(), "age".to_owned(), "active".to_owned()],
                vec!["Alice".to_owned(), "30".to_owned(), "TRUE".to_owned()],
                vec!["Bob".to_owned(), "25".to_owned(), "FALSE".to_owned()],
            ],
        };
        let json = SheetsClient::values_to_json_objects(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
        assert_eq!(parsed[0]["name"], "Alice");
        assert_eq!(parsed[0]["age"], 30);
        assert_eq!(parsed[0]["active"], true);
        assert_eq!(parsed[1]["active"], false);
    }

    #[test]
    fn test_values_to_json_objects_empty_rows() {
        let values = ValueRange {
            range: "Sheet1!A1:B1".to_owned(),
            major_dimension: "ROWS".to_owned(),
            values: vec![
                vec!["col1".to_owned(), "col2".to_owned()],
            ],
        };
        let json = SheetsClient::values_to_json_objects(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_values_to_json_objects_with_types() {
        let values = ValueRange {
            range: "Sheet1!A1:B2".to_owned(),
            major_dimension: "ROWS".to_owned(),
            values: vec![
                vec!["name".to_owned(), "score".to_owned()],
                vec!["Test".to_owned(), "3.14".to_owned()],
            ],
        };
        let json = SheetsClient::values_to_json_objects(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["score"], 3.14);
    }

    #[test]
    fn test_values_to_json_arrays() {
        let values = ValueRange {
            range: "Sheet1!A1:B2".to_owned(),
            major_dimension: "ROWS".to_owned(),
            values: vec![
                vec!["a".to_owned(), "b".to_owned()],
                vec!["1".to_owned(), "2".to_owned()],
            ],
        };
        let json = SheetsClient::values_to_json_arrays(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0], serde_json::json!(["a", "b"]));
        assert_eq!(parsed[1], serde_json::json!(["1", "2"]));
    }

    #[test]
    fn test_value_range_deserialization() {
        let json = r#"{
            "range": "Sheet1!A1:B2",
            "majorDimension": "ROWS",
            "values": [["Name", "Age"], ["Alice", "30"]]
        }"#;
        let values: ValueRange = serde_json::from_str(json).unwrap();
        assert_eq!(values.range, "Sheet1!A1:B2");
        assert_eq!(values.values.len(), 2);
        assert_eq!(values.values[0][0], "Name");
    }

    #[test]
    fn test_spreadsheet_deserialization() {
        let json = r#"{
            "spreadsheetId": "abc123",
            "properties": {"title": "Test Sheet", "locale": "en_US"},
            "sheets": [{
                "properties": {
                    "sheetId": 0,
                    "title": "Sheet1",
                    "index": 0,
                    "sheetType": "GRID",
                    "gridProperties": {"rowCount": 1000, "columnCount": 26}
                }
            }]
        }"#;
        let spreadsheet: Spreadsheet = serde_json::from_str(json).unwrap();
        assert_eq!(spreadsheet.spreadsheet_id, "abc123");
        assert_eq!(spreadsheet.properties.title, "Test Sheet");
        assert_eq!(spreadsheet.sheets.len(), 1);
        assert_eq!(spreadsheet.sheets[0].properties.title, "Sheet1");
        assert_eq!(spreadsheet.sheets[0].properties.grid_properties.row_count, 1000);
    }
}
