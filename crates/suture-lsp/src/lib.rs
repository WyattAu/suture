// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::collapsible_match)]
//! Suture Language Server Protocol implementation.
//!
//! Provides patch-aware editor features:
//! - Blame annotations on hover
//! - File diagnostics (tracked/untracked status)
//! - Merge conflict detection and resolution actions
//! - Hover, completion, and symbols for structured files (JSON/YAML/TOML)
//! - Text document sync (full)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use suture_common::FileStatus;
use suture_core::repository::Repository;

#[derive(Debug)]
pub struct SutureLsp {
    client: Client,
    root_path: Arc<RwLock<Option<PathBuf>>>,
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

impl SutureLsp {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            root_path: Arc::new(RwLock::new(None)),
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn get_repo_and_relative(&self, uri: &Url) -> Option<(Repository, String, PathBuf)> {
        let root = self.root_path.read().await.clone()?;
        let repo = Repository::open(&root).ok()?;
        let file_path = uri.to_file_path().ok()?;
        let relative = file_path.strip_prefix(&root).ok()?;
        let relative_str = relative.to_string_lossy().to_string();
        Some((repo, relative_str, file_path))
    }

    async fn publish_file_diagnostics(&self, uri: &Url) {
        let (relative_str, repo_diagnostics) = {
            let Some((repo, relative_str, _)) = self.get_repo_and_relative(uri).await else {
                return;
            };

            let mut diagnostics: Vec<Diagnostic> = Vec::new();

            let head_tree = repo.snapshot_head().ok();
            let is_tracked = head_tree
                .as_ref()
                .is_some_and(|tree| tree.get(&relative_str).is_some());

            let repo_status = repo.status().ok();
            let file_status = repo_status.as_ref().and_then(|s| {
                s.staged_files
                    .iter()
                    .find(|(p, _)| *p == relative_str)
                    .map(|(_, status)| *status)
            });

            if !is_tracked {
                diagnostics.push(Diagnostic::new_simple(
                    Range::new(Position::new(0, 0), Position::new(0, 1)),
                    format!("File \"{}\" is not tracked by suture", relative_str),
                ));
            } else if let Some(status) = file_status {
                let (severity, label) = match status {
                    FileStatus::Added => (DiagnosticSeverity::HINT, "staged as new"),
                    FileStatus::Modified => (DiagnosticSeverity::HINT, "staged with modifications"),
                    FileStatus::Deleted => (DiagnosticSeverity::WARNING, "staged for deletion"),
                    FileStatus::Clean | FileStatus::Untracked => {
                        (DiagnosticSeverity::INFORMATION, "clean")
                    }
                };
                diagnostics.push(Diagnostic::new(
                    Range::new(Position::new(0, 0), Position::new(0, 1)),
                    Some(severity),
                    Some(NumberOrString::String("suture".to_string())),
                    Some("suture".to_string()),
                    label.to_string(),
                    None,
                    None,
                ));
            }

            drop(repo);

            (relative_str, diagnostics)
        };

        let content_snapshot = {
            let docs = self.documents.read().await;
            docs.get(uri).cloned()
        };

        let mut diagnostics = repo_diagnostics;
        if let Some(content) = &content_snapshot {
            diagnostics.extend(Self::check_merge_conflicts(content));
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "publishing {} diagnostic(s) for {}",
                    diagnostics.len(),
                    relative_str,
                ),
            )
            .await;

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    fn check_merge_conflicts(content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut in_conflict = false;
        let mut conflict_start = 0;

        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("<<<<<<< ") {
                in_conflict = true;
                conflict_start = i;
            } else if line.starts_with(">>>>>>> ") && in_conflict {
                diagnostics.push(Diagnostic::new(
                    Range::new(
                        Position::new(conflict_start as u32, 0),
                        Position::new((i + 1) as u32, 0),
                    ),
                    Some(DiagnosticSeverity::ERROR),
                    Some(NumberOrString::String("merge-conflict".to_string())),
                    Some("suture".to_string()),
                    format!("Merge conflict ({} lines)", i - conflict_start + 1),
                    None,
                    None,
                ));
                in_conflict = false;
            }
        }

        diagnostics
    }

    fn resolve_conflict_action(uri: &Url, range: Range) -> CodeAction {
        CodeAction {
            title: "Resolve with Suture semantic merge".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: None,
            edit: None,
            disabled: None,
            command: Some(Command {
                title: "suture.resolve".to_string(),
                command: "suture.resolveConflict".to_string(),
                arguments: Some(vec![serde_json::json!({
                    "uri": uri.to_string(),
                    "range": {
                        "start": { "line": range.start.line, "character": range.start.character },
                        "end": { "line": range.end.line, "character": range.end.character },
                    },
                })]),
            }),
            is_preferred: Some(true),
            data: None,
        }
    }

    fn hover_structured(uri: &Url, position: Position, content: &str) -> Option<Hover> {
        let ext = std::path::Path::new(uri.path())
            .extension()?
            .to_str()?
            .to_lowercase();
        match ext.as_str() {
            "json" => Self::hover_json(content, position),
            "yaml" | "yml" => Self::hover_yaml(content, position),
            "toml" => Self::hover_toml(content, position),
            _ => None,
        }
    }

    fn hover_json(content: &str, position: Position) -> Option<Hover> {
        let value: serde_json::Value = serde_json::from_str(content).ok()?;
        let line_content = content.lines().nth(position.line as usize)?;
        let key = extract_json_key(line_content)?;
        let val = value.get(&key)?;
        let type_name = get_json_value_type(val);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**{}**: `{}`", key, type_name),
            }),
            range: None,
        })
    }

    fn hover_yaml(content: &str, position: Position) -> Option<Hover> {
        let line_content = content.lines().nth(position.line as usize)?;
        let trimmed = line_content.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('-')
            || trimmed.starts_with('[')
        {
            return None;
        }
        let (key, value_part) = trimmed.split_once(':')?;
        let key = key.trim();
        if key.is_empty() {
            return None;
        }
        let value = value_part.trim();
        let type_name = infer_yaml_type(value);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**{}**: `{}`", key, type_name),
            }),
            range: None,
        })
    }

    fn hover_toml(content: &str, position: Position) -> Option<Hover> {
        let table: toml::Table = toml::from_str(content).ok()?;
        let line_content = content.lines().nth(position.line as usize)?;
        let trimmed = line_content.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            return None;
        }
        let (key, _value_part) = trimmed.split_once('=')?;
        let key = key.trim();
        if key.is_empty() {
            return None;
        }

        let section = current_toml_section(content, position.line as usize);
        let val = get_toml_value(&table, &section, key)?;
        let type_name = get_toml_value_type(val);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**{}**: `{}`", key, type_name),
            }),
            range: None,
        })
    }

    fn complete_structured(content: &str, position: Position, ext: &str) -> Vec<CompletionItem> {
        match ext {
            "json" => Self::complete_json(content, position),
            "yaml" | "yml" => Self::complete_yaml(content),
            "toml" => Self::complete_toml(content),
            _ => Vec::new(),
        }
    }

    fn complete_json(content: &str, _position: Position) -> Vec<CompletionItem> {
        let value: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let object = match value.as_object() {
            Some(o) => o,
            None => return Vec::new(),
        };

        object
            .keys()
            .map(|key| {
                let ty = get_json_value_type(object.get(key).unwrap_or(&serde_json::Value::Null));
                CompletionItem {
                    label: format!("\"{}\"", key),
                    kind: Some(CompletionItemKind::PROPERTY),
                    detail: Some(format!("`{}`", ty)),
                    insert_text: Some(format!("\"{}\"", key)),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn complete_yaml(content: &str) -> Vec<CompletionItem> {
        let mut seen = HashMap::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with('-')
                || trimmed.starts_with('[')
            {
                continue;
            }
            if let Some((key, _)) = trimmed.split_once(':') {
                let key = key.trim().to_string();
                if !key.is_empty() {
                    seen.entry(key).or_insert(true);
                }
            }
        }

        seen
            .into_keys()
            .map(|key| CompletionItem {
                label: key.clone(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("YAML key".to_string()),
                insert_text: Some(key),
                ..Default::default()
            })
            .collect()
    }

    fn complete_toml(content: &str) -> Vec<CompletionItem> {
        let table: toml::Table = match toml::from_str(content) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let mut items = Vec::new();
        collect_toml_keys(&table, String::new(), &mut items);
        items
    }

    fn document_symbols_structured(content: &str, ext: &str) -> Vec<DocumentSymbol> {
        match ext {
            "json" => Self::document_symbols_json(content),
            "yaml" | "yml" => Self::document_symbols_yaml(content),
            "toml" => Self::document_symbols_toml(content),
            _ => Vec::new(),
        }
    }

    fn document_symbols_json(content: &str) -> Vec<DocumentSymbol> {
        let value: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let object = match value.as_object() {
            Some(o) => o,
            None => return Vec::new(),
        };

        object
            .iter()
            .map(|(key, val)| {
                let range = find_key_range_in_content(content, key);
                DocumentSymbol {
                    name: key.clone(),
                    detail: Some(get_json_value_type(val)),
                    kind: match val {
                        serde_json::Value::Object(_) => SymbolKind::OBJECT,
                        serde_json::Value::Array(_) => SymbolKind::ARRAY,
                        _ => SymbolKind::PROPERTY,
                    },
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                }
            })
            .collect()
    }

    fn document_symbols_yaml(content: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with('-')
                || trimmed.starts_with('[')
            {
                continue;
            }
            if let Some((key, value_part)) = trimmed.split_once(':') {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                let value = value_part.trim();
                let kind = if value.is_empty() || value.starts_with('\n') {
                    SymbolKind::OBJECT
                } else if value.starts_with('[') {
                    SymbolKind::ARRAY
                } else {
                    SymbolKind::PROPERTY
                };
                let character = line.find(key).unwrap_or(0) as u32;
                let range = Range::new(
                    Position::new(i as u32, character),
                    Position::new(i as u32, character + key.len() as u32),
                );
                symbols.push(DocumentSymbol {
                    name: key.to_string(),
                    detail: Some(infer_yaml_type(value)),
                    kind,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                });
            }
        }
        symbols
    }

    fn document_symbols_toml(content: &str) -> Vec<DocumentSymbol> {
        let table: toml::Table = match toml::from_str(content) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let mut symbols = Vec::new();
        collect_toml_symbols(&table, content, String::new(), &mut symbols);
        symbols
    }
}

fn collect_toml_symbols(
    table: &toml::Table,
    content: &str,
    prefix: String,
    symbols: &mut Vec<DocumentSymbol>,
) {
    for (key, value) in table {
        let display_name = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        let range = find_toml_key_range(content, &display_name);
        symbols.push(DocumentSymbol {
            name: display_name.clone(),
            detail: Some(get_toml_value_type(value)),
            kind: match value {
                toml::Value::Table(_) => SymbolKind::OBJECT,
                toml::Value::Array(_) => SymbolKind::ARRAY,
                _ => SymbolKind::PROPERTY,
            },
            range,
            selection_range: range,
            children: None,
            tags: None,
            #[allow(deprecated)]
            deprecated: None,
        });
        if let Some(nested) = value.as_table() {
            collect_toml_symbols(nested, content, display_name, symbols);
        }
    }
}

fn extract_json_key(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    if end > 0 {
        Some(rest[..end].to_string())
    } else {
        None
    }
}

fn get_json_value_type(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(arr) => format!("array[{}]", arr.len()),
        serde_json::Value::Object(obj) => format!("object{{{}}}", obj.len()),
    }
}

fn infer_yaml_type(value: &str) -> String {
    if value.is_empty() || value.starts_with('|') || value.starts_with('>') {
        "mapping / multiline".to_string()
    } else if value == "true" || value == "false" {
        "boolean".to_string()
    } else if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        "number".to_string()
    } else if value.starts_with('"') || value.starts_with('\'') {
        "string".to_string()
    } else if value.starts_with('[') {
        "array".to_string()
    } else if value.starts_with('{') {
        "object".to_string()
    } else {
        "string".to_string()
    }
}

fn current_toml_section(content: &str, target_line: usize) -> String {
    let mut current = String::new();
    for (i, line) in content.lines().enumerate() {
        if i >= target_line {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                current = inner.trim().to_string();
            }
        }
    }
    current
}

fn get_toml_value<'a>(
    root: &'a toml::Table,
    section: &str,
    key: &str,
) -> Option<&'a toml::Value> {
    let mut table = root;
    if !section.is_empty() {
        for part in section.split('.') {
            let value = table.get(part)?;
            table = value.as_table()?;
        }
    }
    table.get(key)
}

fn get_toml_value_type(value: &toml::Value) -> String {
    match value {
        toml::Value::String(_) => "string".to_string(),
        toml::Value::Integer(_) => "integer".to_string(),
        toml::Value::Float(_) => "float".to_string(),
        toml::Value::Boolean(_) => "boolean".to_string(),
        toml::Value::Array(arr) => format!("array[{}]", arr.len()),
        toml::Value::Table(tbl) => format!("table{{{}}}", tbl.len()),
        toml::Value::Datetime(_) => "datetime".to_string(),
    }
}

fn find_key_range_in_content(content: &str, key: &str) -> Range {
    let search = format!("\"{}\"", key);
    if let Some(byte_offset) = content.find(&search) {
        let before = &content[..byte_offset];
        let line = before.matches('\n').count() as u32;
        let last_newline = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let character = (byte_offset - last_newline) as u32;
        let end_character = character + search.len() as u32;
        Range::new(
            Position::new(line, character),
            Position::new(line, end_character),
        )
    } else {
        Range::new(Position::new(0, 0), Position::new(0, 1))
    }
}

fn find_toml_key_range(content: &str, full_key: &str) -> Range {
    let leaf = full_key.split('.').last().unwrap_or(full_key);
    let search = format!("{} =", leaf);
    if let Some(byte_offset) = content.find(&search) {
        let before = &content[..byte_offset];
        let line = before.matches('\n').count() as u32;
        let last_newline = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let character = (byte_offset - last_newline) as u32;
        let end_character = character + leaf.len() as u32;
        Range::new(
            Position::new(line, character),
            Position::new(line, end_character),
        )
    } else {
        Range::new(Position::new(0, 0), Position::new(0, 1))
    }
}

fn collect_toml_keys(
    table: &toml::Table,
    prefix: String,
    items: &mut Vec<CompletionItem>,
) {
    for (key, value) in table {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        items.push(CompletionItem {
            label: full_key.clone(),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(format!("`{}`", get_toml_value_type(value))),
            insert_text: Some(full_key.clone()),
            ..Default::default()
        });
        if let Some(nested) = value.as_table() {
            collect_toml_keys(nested, full_key, items);
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SutureLsp {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri
            && let Ok(path) = root_uri.to_file_path()
        {
            *self.root_path.write().await = Some(path);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ":".to_string(),
                        "=".to_string(),
                        "\"".to_string(),
                    ]),
                    ..Default::default()
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "suture-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Suture LSP initialized".to_string())
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        self.documents
            .write()
            .await
            .insert(uri.clone(), params.text_document.text);
        self.publish_file_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.documents.write().await.insert(uri, change.text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        self.publish_file_diagnostics(&uri).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(content) = self.documents.read().await.get(uri) {
            if let Some(hover) = Self::hover_structured(uri, position, content) {
                return Ok(Some(hover));
            }
        }

        let Some((repo, relative_str, _)) = self.get_repo_and_relative(uri).await else {
            return Ok(None);
        };

        let line = position.line as usize + 1;

        let blame_entries = match repo.blame(&relative_str, None) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };

        if let Some(entry) = blame_entries.iter().find(|e| e.line_number == line) {
            let contents = format!(
                "**Patch**: {}\n**Author**: {}\n**Message**: {}",
                &entry.patch_id.to_hex()[..12],
                entry.author,
                entry.message,
            );

            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: contents,
                }),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;

        let has_merge_conflict = params.context.diagnostics.iter().any(|d| {
            matches!(
                &d.code,
                Some(NumberOrString::String(code)) if code == "merge-conflict"
            )
        });

        if !has_merge_conflict {
            return Ok(None);
        }

        let actions: Vec<CodeActionOrCommand> = params
            .context
            .diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    &d.code,
                    Some(NumberOrString::String(code)) if code == "merge-conflict"
                )
            })
            .map(|d| CodeActionOrCommand::CodeAction(Self::resolve_conflict_action(uri, d.range)))
            .collect();

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let content = match self.documents.read().await.get(uri) {
            Some(c) => c.clone(),
            None => return Ok(None),
        };

        let ext = match std::path::Path::new(uri.path())
            .extension()
            .and_then(|e| e.to_str())
        {
            Some(e) => e.to_lowercase(),
            None => return Ok(None),
        };

        let items = Self::complete_structured(&content, position, &ext);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        let content = match self.documents.read().await.get(uri) {
            Some(c) => c.clone(),
            None => return Ok(None),
        };

        let ext = match std::path::Path::new(uri.path())
            .extension()
            .and_then(|e| e.to_str())
        {
            Some(e) => e.to_lowercase(),
            None => return Ok(None),
        };

        let symbols = Self::document_symbols_structured(&content, &ext);
        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DocumentSymbolResponse::Nested(symbols)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::LspService;

    fn create_service() -> LspService<SutureLsp> {
        let (service, _) = LspService::new(SutureLsp::new);
        service
    }

    #[test]
    fn test_new_does_not_panic() {
        let _service = create_service();
    }

    #[tokio::test]
    async fn test_initialize_capabilities() {
        let service = create_service();
        let server = service.inner();

        let params = InitializeParams {
            root_uri: Some(Url::parse("file:///tmp/suture-test").unwrap()),
            ..Default::default()
        };

        let result = server.initialize(params).await.unwrap();
        let caps = result.capabilities;

        assert!(
            matches!(
                caps.hover_provider,
                Some(HoverProviderCapability::Simple(true))
            ),
            "hover should be enabled"
        );
        assert!(
            caps.definition_provider.is_none(),
            "definition_provider should not be declared"
        );
        assert!(
            caps.completion_provider.is_some(),
            "completion_provider should be declared"
        );
        assert!(
            caps.code_action_provider.is_some(),
            "code_action_provider should be declared"
        );
        assert!(
            caps.document_symbol_provider.is_some(),
            "document_symbol_provider should be declared"
        );
        assert!(
            caps.semantic_tokens_provider.is_none(),
            "semantic_tokens_provider should not be declared"
        );
        assert!(
            caps.references_provider.is_none(),
            "references_provider should not be declared"
        );
        assert_eq!(result.server_info.as_ref().unwrap().name, "suture-lsp");
    }

    #[tokio::test]
    async fn test_initialize_without_root_uri() {
        let service = create_service();
        let server = service.inner();

        let params = InitializeParams::default();
        let result = server.initialize(params).await.unwrap();

        assert!(result.capabilities.hover_provider.is_some());
        assert!(server.root_path.read().await.is_none());
    }

    #[tokio::test]
    async fn test_initialize_with_root_sets_path() {
        let dir = tempfile::tempdir().unwrap();
        let service = create_service();
        let server = service.inner();

        let root_uri = Url::from_directory_path(dir.path()).unwrap();
        let params = InitializeParams {
            root_uri: Some(root_uri),
            ..Default::default()
        };
        server.initialize(params).await.unwrap();

        let stored = server.root_path.read().await;
        assert!(stored.is_some());
        assert_eq!(stored.as_ref().unwrap(), dir.path());
    }

    fn setup_repo_with_file(
        dir: &std::path::Path,
        filename: &str,
        content: &str,
    ) -> (Repository, std::path::PathBuf) {
        let mut repo = Repository::init(dir, "Test Author").unwrap();
        let file_path = dir.join(filename);
        std::fs::write(&file_path, content).unwrap();
        repo.add(filename).unwrap();
        repo.commit("test commit").unwrap();
        (repo, file_path)
    }

    async fn init_service_with_root(
        dir: &std::path::Path,
    ) -> (LspService<SutureLsp>, std::path::PathBuf) {
        let service = create_service();
        let server = service.inner();
        let root_uri = Url::from_directory_path(dir).unwrap();
        let params = InitializeParams {
            root_uri: Some(root_uri),
            ..Default::default()
        };
        server.initialize(params).await.unwrap();
        (service, dir.to_path_buf())
    }

    #[tokio::test]
    async fn test_hover_blame_info() {
        let dir = tempfile::tempdir().unwrap();
        let content = "hello world\nsecond line\n";
        setup_repo_with_file(dir.path(), "test.txt", content);

        let (service, _) = init_service_with_root(dir.path()).await;
        let server = service.inner();
        let file_uri = Url::from_file_path(dir.path().join("test.txt")).unwrap();

        let result = server
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: file_uri },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .unwrap();

        assert!(
            result.is_some(),
            "hover should return blame info for committed file"
        );
        let hover = result.unwrap();
        if let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents {
            assert!(value.contains("Test Author"), "should contain author");
            assert!(
                value.contains("test commit"),
                "should contain commit message"
            );
            assert!(value.contains("**Patch**:"), "should contain patch header");
        } else {
            panic!("expected Markdown hover content");
        }
    }

    #[tokio::test]
    async fn test_hover_no_repo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "some content\n").unwrap();

        let (service, _) = LspService::new(SutureLsp::new);
        let server = service.inner();

        let root_uri = Url::from_directory_path(dir.path()).unwrap();
        let params = InitializeParams {
            root_uri: Some(root_uri),
            ..Default::default()
        };
        server.initialize(params).await.unwrap();

        let file_uri = Url::from_file_path(dir.path().join("test.txt")).unwrap();
        let result = server
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: file_uri },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .unwrap();

        assert!(
            result.is_none(),
            "hover should return None when no repo exists"
        );
    }

    #[tokio::test]
    async fn test_hover_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        setup_repo_with_file(dir.path(), "empty.txt", "");

        let (service, _) = init_service_with_root(dir.path()).await;
        let server = service.inner();
        let file_uri = Url::from_file_path(dir.path().join("empty.txt")).unwrap();

        let result = server
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: file_uri },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .unwrap();

        assert!(
            result.is_none(),
            "hover should return None for empty file (no blame entries)"
        );
    }

    #[tokio::test]
    async fn test_hover_outside_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("other.txt");
        std::fs::write(&outside_file, "outside content\n").unwrap();

        let (service, _) = init_service_with_root(workspace.path()).await;
        let server = service.inner();
        let file_uri = Url::from_file_path(&outside_file).unwrap();

        let result = server
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: file_uri },
                    position: Position::new(0, 0),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .unwrap();

        assert!(
            result.is_none(),
            "hover should return None for files outside workspace"
        );
    }

    #[tokio::test]
    async fn test_diagnostics_untracked_file() {
        let dir = tempfile::tempdir().unwrap();
        Repository::init(dir.path(), "Test Author").unwrap();
        std::fs::write(dir.path().join("untracked.txt"), "new file\n").unwrap();

        let (service, _) = LspService::new(SutureLsp::new);
        let server = service.inner();
        let root_uri = Url::from_directory_path(dir.path()).unwrap();
        let params = InitializeParams {
            root_uri: Some(root_uri),
            ..Default::default()
        };
        server.initialize(params).await.unwrap();

        let file_uri = Url::from_file_path(dir.path().join("untracked.txt")).unwrap();
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri,
                language_id: "plaintext".to_string(),
                version: 1,
                text: "new file\n".to_string(),
            },
        };

        server.did_open(open_params).await;
    }

    #[tokio::test]
    async fn test_diagnostics_modified_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("tracked.txt");
        std::fs::write(&file_path, "original content\n").unwrap();

        let mut repo = Repository::init(dir.path(), "Test Author").unwrap();
        repo.add("tracked.txt").unwrap();
        repo.commit("initial").unwrap();

        std::fs::write(&file_path, "modified content\n").unwrap();
        repo.add("tracked.txt").unwrap();

        let (service, _) = LspService::new(SutureLsp::new);
        let server = service.inner();
        let root_uri = Url::from_directory_path(dir.path()).unwrap();
        let params = InitializeParams {
            root_uri: Some(root_uri),
            ..Default::default()
        };
        server.initialize(params).await.unwrap();

        let file_uri = Url::from_file_path(&file_path).unwrap();
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri,
                language_id: "plaintext".to_string(),
                version: 1,
                text: "modified content\n".to_string(),
            },
        };

        server.did_open(open_params).await;
    }

    #[tokio::test]
    async fn test_diagnostics_empty_repo() {
        let dir = tempfile::tempdir().unwrap();
        Repository::init(dir.path(), "Test Author").unwrap();

        let (service, _) = LspService::new(SutureLsp::new);
        let server = service.inner();
        let root_uri = Url::from_directory_path(dir.path()).unwrap();
        let params = InitializeParams {
            root_uri: Some(root_uri),
            ..Default::default()
        };
        server.initialize(params).await.unwrap();

        let file_uri = Url::from_file_path(dir.path().join("nonexistent.txt")).unwrap();
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri,
                language_id: "plaintext".to_string(),
                version: 1,
                text: "anything\n".to_string(),
            },
        };

        server.did_open(open_params).await;
    }

    #[test]
    fn test_check_merge_conflicts() {
        let content = "line1\n<<<<<<< ours\nour line\n=======\ntheir line\n>>>>>>> theirs\nline2\n";
        let diagnostics = SutureLsp::check_merge_conflicts(content);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].range.start.line, 1);
        assert_eq!(diagnostics[0].range.end.line, 6);
        assert_eq!(diagnostics[0].message, "Merge conflict (5 lines)");
        if let Some(NumberOrString::String(code)) = &diagnostics[0].code {
            assert_eq!(code, "merge-conflict");
        } else {
            panic!("expected string code");
        }
    }

    #[test]
    fn test_check_merge_conflicts_none() {
        let content = "clean file\nno conflicts\n";
        let diagnostics = SutureLsp::check_merge_conflicts(content);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_check_merge_conflicts_multiple() {
        let content = "a\n<<<<<<< head\nx\n=======\ny\n>>>>>>> branch\nb\n<<<<<<< head\nx\n=======\ny\n>>>>>>> branch2\n";
        let diagnostics = SutureLsp::check_merge_conflicts(content);
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn test_hover_json() {
        let content = r#"{"name": "test", "count": 42}"#;
        let hover = SutureLsp::hover_json(content, Position::new(0, 1));
        assert!(hover.is_some());
        let hover = hover.unwrap();
        if let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents {
            assert!(value.contains("**name**"), "value: {}", value);
            assert!(value.contains("`string`"), "value: {}", value);
        } else {
            panic!("expected markdown");
        }
    }

    #[test]
    fn test_hover_toml() {
        let content = r#"[server]
port = 8080
host = "localhost"
"#;
        let hover = SutureLsp::hover_toml(content, Position::new(1, 0));
        assert!(hover.is_some());
        let hover = hover.unwrap();
        if let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents {
            assert!(value.contains("**port**"), "value: {}", value);
            assert!(value.contains("`integer`"), "value: {}", value);
        } else {
            panic!("expected markdown");
        }
    }

    #[test]
    fn test_hover_yaml() {
        let content = "name: test\ncount: 42\nactive: true\n";
        let hover = SutureLsp::hover_yaml(content, Position::new(0, 0));
        assert!(hover.is_some());
        let hover = hover.unwrap();
        if let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents {
            assert!(value.contains("**name**"), "value: {}", value);
            assert!(value.contains("`string`"), "value: {}", value);
        } else {
            panic!("expected markdown");
        }
    }

    #[test]
    fn test_complete_json() {
        let content = r#"{"name": "test", "count": 42}"#;
        let items = SutureLsp::complete_json(content, Position::new(0, 0));
        assert_eq!(items.len(), 2);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&r#""name""#));
        assert!(labels.contains(&r#""count""#));
    }

    #[test]
    fn test_complete_toml() {
        let content = r#"[server]
port = 8080
"#;
        let items = SutureLsp::complete_toml(content);
        assert!(!items.is_empty());
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"server.port"));
    }

    #[test]
    fn test_complete_yaml() {
        let content = "name: test\ncount: 42\n";
        let items = SutureLsp::complete_yaml(content);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_document_symbols_json() {
        let content = r#"{"name": "test", "items": [1, 2]}"#;
        let symbols = SutureLsp::document_symbols_json(content);
        assert_eq!(symbols.len(), 2);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"name"));
        assert!(names.contains(&"items"));
        let name_sym = symbols.iter().find(|s| s.name == "name").unwrap();
        assert_eq!(name_sym.kind, SymbolKind::PROPERTY);
        let items_sym = symbols.iter().find(|s| s.name == "items").unwrap();
        assert_eq!(items_sym.kind, SymbolKind::ARRAY);
    }

    #[test]
    fn test_document_symbols_toml() {
        let content = r#"[server]
port = 8080
"#;
        let symbols = SutureLsp::document_symbols_toml(content);
        assert!(!symbols.is_empty());
    }

    #[test]
    fn test_document_symbols_yaml() {
        let content = "name: test\nitems:\n  - one\n  - two\n";
        let symbols = SutureLsp::document_symbols_yaml(content);
        assert!(!symbols.is_empty());
    }

    #[test]
    fn test_extract_json_key() {
        assert_eq!(extract_json_key(r#"  "hello": "world","#), Some("hello".to_string()));
        assert_eq!(extract_json_key(r#""key": 123"#), Some("key".to_string()));
        assert_eq!(extract_json_key("no key here"), None);
    }

    #[test]
    fn test_get_json_value_type() {
        use serde_json::json;
        assert_eq!(get_json_value_type(&json!(null)), "null");
        assert_eq!(get_json_value_type(&json!(true)), "boolean");
        assert_eq!(get_json_value_type(&json!(42)), "number");
        assert_eq!(get_json_value_type(&json!("hello")), "string");
        assert_eq!(get_json_value_type(&json!([1, 2])), "array[2]");
        assert_eq!(get_json_value_type(&json!({"a": 1})), "object{1}");
    }
}
