//! Suture Language Server Protocol implementation.
//!
//! Provides semantic navigation and patch-aware features:
//! - Blame annotations on hover
//! - Go-to-definition pointing to the introducing patch
//! - File diagnostics (staged/untracked status)
//! - Semantic tokens (placeholder)

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
}

impl SutureLsp {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            root_path: Arc::new(RwLock::new(None)),
        }
    }

    async fn get_repo_and_relative(
        &self,
        uri: &Url,
    ) -> Option<(Repository, String, PathBuf)> {
        let root = self.root_path.read().await.clone()?;
        let repo = Repository::open(&root).ok()?;
        let file_path = uri.to_file_path().ok()?;
        let relative = file_path.strip_prefix(&root).ok()?;
        let relative_str = relative.to_string_lossy().to_string();
        Some((repo, relative_str, file_path))
    }

    async fn publish_file_diagnostics(&self, uri: &Url) {
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

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
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
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::VARIABLE,
                                    SemanticTokenType::FUNCTION,
                                    SemanticTokenType::CLASS,
                                ],
                                token_modifiers: vec![],
                            },
                            full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                            ..Default::default()
                        },
                    ),
                ),
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
        let uri = params.text_document.uri;
        self.publish_file_diagnostics(&uri).await;
    }

    async fn did_change(&self, _params: DidChangeTextDocumentParams) {}

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        self.publish_file_diagnostics(&uri).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let Some((repo, relative_str, _)) = self
            .get_repo_and_relative(
                &params.text_document_position_params.text_document.uri,
            )
            .await
        else {
            return Ok(None);
        };

        let line = params.text_document_position_params.position.line as usize + 1;

        let blame_entries = match repo.blame(&relative_str) {
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

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let Some((repo, relative_str, file_path)) = self
            .get_repo_and_relative(
                &params.text_document_position_params.text_document.uri,
            )
            .await
        else {
            return Ok(None);
        };

        let line = params.text_document_position_params.position.line as usize + 1;

        let blame_entries = match repo.blame(&relative_str) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };

        if let Some(entry) = blame_entries.iter().find(|e| e.line_number == line) {
            let patch_hex = entry.patch_id.to_hex();
            let short_id = &patch_hex[..12];

            let target_uri = Url::parse(&format!("suture://patch/{}", patch_hex)).ok();

            let fallback_uri =
                Url::from_file_path(&file_path).unwrap_or_else(|_| {
                    params
                        .text_document_position_params
                        .text_document
                        .uri
                        .clone()
                });

            let location = Location {
                uri: fallback_uri,
                range: Range::new(
                    Position::new((entry.line_number - 1) as u32, 0),
                    Position::new(entry.line_number as u32, 0),
                ),
            };

            if let Some(target_uri) = target_uri {
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!(
                            "Go-to-definition: line {} introduced by patch {} in \"{}\"",
                            line, short_id, entry.message
                        ),
                    )
                    .await;

                let origin_selection_range = Range::new(
                    Position::new((entry.line_number - 1) as u32, 0),
                    Position::new(entry.line_number as u32, 0),
                );

                let link = LocationLink {
                    origin_selection_range: Some(origin_selection_range),
                    target_uri,
                    target_range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    target_selection_range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                };

                return Ok(Some(GotoDefinitionResponse::Link(vec![link])));
            }

            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }

        Ok(None)
    }

    async fn semantic_tokens_full(
        &self,
        _params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        Ok(None)
    }
}
