//! Suture Language Server Protocol implementation.
//!
//! Provides semantic navigation and patch-aware features:
//! - Blame annotations on hover
//! - Patch history for file regions
//! - Go-to-definition across patch boundaries

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

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
                completion_provider: Some(CompletionOptions::default()),
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
        let _ = params;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let _ = params;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let _ = params;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let root = self.root_path.read().await.clone();
        let root = match root {
            Some(r) => r,
            None => return Ok(None),
        };

        let repo = match Repository::open(&root) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        let file_path = match params
            .text_document_position_params
            .text_document
            .uri
            .to_file_path()
        {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let relative = match file_path.strip_prefix(&root) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        let relative_str = relative.to_string_lossy().to_string();

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
}
