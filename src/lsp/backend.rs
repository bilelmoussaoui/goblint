use std::{collections::HashMap, path::PathBuf, sync::Arc};

use goblint::{ast_context::AstContext, config::Config, scanner};
use tokio::sync::Mutex;
use tower_lsp::{Client, LanguageServer, jsonrpc::Result, lsp_types::*};

fn byte_offset_to_lsp_position(content: &[u8], offset: usize) -> Option<Position> {
    if offset > content.len() {
        return None;
    }
    let mut line = 0u32;
    let mut character = 0u32;
    for (i, &byte) in content.iter().enumerate() {
        if i == offset {
            return Some(Position { line, character });
        }
        if byte == b'\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    Some(Position { line, character })
}

pub struct GObjectBackend {
    client: Client,
    documents: Arc<Mutex<HashMap<Url, String>>>,
    workspace_root: Arc<Mutex<Option<PathBuf>>>,
    ast_context: Arc<Mutex<Option<AstContext>>>,
    config: Arc<Mutex<Option<Config>>>,
}

impl GObjectBackend {
    pub fn new(client: Client) -> Self {
        GObjectBackend {
            client,
            documents: Arc::new(Mutex::new(HashMap::new())),
            workspace_root: Arc::new(Mutex::new(None)),
            ast_context: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize workspace (find root, load config, build AST context)
    async fn initialize_workspace(&self, file_path: &std::path::Path) -> Result<()> {
        // Find workspace root by looking for goblint.toml
        let mut current = file_path;
        let mut root = None;
        while let Some(parent) = current.parent() {
            let config_path = parent.join("goblint.toml");
            if config_path.exists() {
                root = Some(parent.to_path_buf());
                break;
            }
            current = parent;
        }

        let workspace_root =
            root.unwrap_or_else(|| file_path.parent().unwrap_or(file_path).to_path_buf());

        // Load config
        let config_path = workspace_root.join("goblint.toml");
        let config = if config_path.exists() {
            match Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    self.client
                        .log_message(MessageType::ERROR, format!("Failed to load config: {}", e))
                        .await;
                    return Ok(());
                }
            }
        } else {
            Config::default()
        };

        // Build ignore matcher
        let ignore_matcher = match config.build_ignore_matcher() {
            Ok(m) => m,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("Failed to build ignore matcher: {}", e),
                    )
                    .await;
                return Ok(());
            }
        };

        // Get header visibility from meson introspection (for dead code analysis)
        let meson_headers =
            goblint::meson::get_header_sets(&workspace_root, config.build_dir.as_deref())
                .ok()
                .flatten();

        // Build AST context
        let ast_context = match AstContext::build_with_ignore(
            &workspace_root,
            &ignore_matcher,
            None,
            meson_headers,
        ) {
            Ok(ctx) => ctx,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("Failed to build AST context: {}", e),
                    )
                    .await;
                return Ok(());
            }
        };

        // Store in state
        *self.workspace_root.lock().await = Some(workspace_root);
        *self.config.lock().await = Some(config);
        *self.ast_context.lock().await = Some(ast_context);

        self.client
            .log_message(MessageType::INFO, "Workspace initialized")
            .await;

        Ok(())
    }

    async fn lint_document(&self, uri: &Url) -> Result<()> {
        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;

        // Initialize workspace if not already done
        if self.workspace_root.lock().await.is_none() {
            self.initialize_workspace(&path).await?;
        }

        // Update the specific file in AST context
        if let Some(ast_context) = self.ast_context.lock().await.as_mut()
            && let Err(e) = ast_context.update_file(&path)
        {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("Failed to update file {}: {}", path.display(), e),
                )
                .await;
        }

        // Get workspace root and config
        let workspace_root = match self.workspace_root.lock().await.as_ref() {
            Some(root) => root.clone(),
            None => return Ok(()),
        };

        let config = match self.config.lock().await.as_ref() {
            Some(cfg) => cfg.clone(),
            None => return Ok(()),
        };

        // Run scanner with locked AST context
        let ast_context_guard = self.ast_context.lock().await;
        let violations = match ast_context_guard.as_ref() {
            Some(ctx) => match scanner::scan_with_ast(ctx, &config, &workspace_root, None) {
                Ok(v) => v,
                Err(e) => {
                    self.client
                        .log_message(MessageType::ERROR, format!("Failed to scan: {}", e))
                        .await;
                    return Ok(());
                }
            },
            None => return Ok(()),
        };

        // Convert violations to diagnostics
        let diagnostics: Vec<Diagnostic> = violations
            .iter()
            .filter(|v| v.file == path)
            .map(|v| {
                let range = Range {
                    start: Position {
                        line: v.line.saturating_sub(1) as u32,
                        character: v.column.saturating_sub(1) as u32,
                    },
                    end: Position {
                        line: v.line.saturating_sub(1) as u32,
                        character: v.column.saturating_sub(1) as u32 + 1,
                    },
                };

                Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String(v.rule.to_string())),
                    source: Some(env!("CARGO_PKG_NAME").to_owned()),
                    message: v.message.clone(),
                    data: if v.fixes.is_empty() {
                        None
                    } else {
                        serde_json::to_value(&v.fixes).ok()
                    },
                    ..Default::default()
                }
            })
            .collect();

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
        Ok(())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for GObjectBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "GObject LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        self.documents.lock().await.insert(uri.clone(), text);
        let _ = self.lint_document(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().next() {
            self.documents.lock().await.insert(uri.clone(), change.text);
            let _ = self.lint_document(&uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let _ = self.lint_document(&params.text_document.uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .lock()
            .await
            .remove(&params.text_document.uri);
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let path = match params.text_document.uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let content = match std::fs::read(&path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let mut actions: CodeActionResponse = Vec::new();

        for diagnostic in &params.context.diagnostics {
            let fixes: Vec<goblint::rules::Fix> = match &diagnostic.data {
                Some(data) => match serde_json::from_value(data.clone()) {
                    Ok(f) => f,
                    Err(_) => continue,
                },
                None => continue,
            };

            let mut text_edits = Vec::new();
            for fix in &fixes {
                if let (Some(start), Some(end)) = (
                    byte_offset_to_lsp_position(&content, fix.start_byte),
                    byte_offset_to_lsp_position(&content, fix.end_byte),
                ) {
                    text_edits.push(TextEdit {
                        range: Range { start, end },
                        new_text: fix.replacement.clone(),
                    });
                }
            }

            if text_edits.is_empty() {
                continue;
            }

            let rule_name = match &diagnostic.code {
                Some(NumberOrString::String(s)) => s.as_str(),
                _ => env!("CARGO_PKG_NAME"),
            };

            let mut changes = HashMap::new();
            changes.insert(params.text_document.uri.clone(), text_edits);

            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: format!("Fix: {}", rule_name),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    ..Default::default()
                }),
                is_preferred: Some(true),
                ..Default::default()
            }));
        }

        Ok(if actions.is_empty() {
            None
        } else {
            Some(actions)
        })
    }
}
