//! Suture Language Server Protocol implementation.
//!
//! Provides patch-aware editor features:
//! - Blame annotations on hover
//! - File diagnostics (tracked/untracked status)
//! - Text document sync (full)

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

    async fn did_change(&self, _params: DidChangeTextDocumentParams) {
        // Intentional no-op: diagnostics are published on did_open and did_save
        // rather than on every keystroke, since status does not depend on buffer content.
    }

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
            matches!(caps.hover_provider, Some(HoverProviderCapability::Simple(true))),
            "hover should be enabled"
        );
        assert!(caps.definition_provider.is_none(), "definition_provider should not be declared");
        assert!(caps.completion_provider.is_none(), "completion_provider should not be declared");
        assert!(caps.semantic_tokens_provider.is_none(), "semantic_tokens_provider should not be declared");
        assert!(caps.references_provider.is_none(), "references_provider should not be declared");
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

        assert!(result.is_some(), "hover should return blame info for committed file");
        let hover = result.unwrap();
        if let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents {
            assert!(value.contains("Test Author"), "should contain author");
            assert!(value.contains("test commit"), "should contain commit message");
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

        assert!(result.is_none(), "hover should return None when no repo exists");
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
}
