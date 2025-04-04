mod message_state;
mod handlers {
    pub(crate) mod notification;
    pub(crate) mod request;
}
mod utils;

use lsp_server::{Connection, Message};
use vfs::{FileSystem, MemoryFS, OverlayFS, PhysicalFS, VfsPath};

use crate::message_state::MessageState;

fn main() -> anyhow::Result<()> {
    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    use handlers::request::Command;
    use lsp_types::{
        CodeActionProviderCapability, ExecuteCommandOptions, HoverProviderCapability,
        ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    };
    let hover_provider = Some(HoverProviderCapability::Simple(true));
    let text_document_sync = Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL));
    let code_action_provider = Some(CodeActionProviderCapability::Simple(true));
    let execute_command_provider = Some(ExecuteCommandOptions {
        commands: vec![
            Command::StageFile.to_str().to_string(),
            Command::UnstageFile.to_str().to_string(),
        ],
        ..Default::default()
    });
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        hover_provider,
        text_document_sync,
        code_action_provider,
        execute_command_provider,
        ..Default::default()
    })
    .unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    let fs = OverlayFS::new(&[
        VfsPath::new(MemoryFS::new()),
        VfsPath::new(PhysicalFS::new("/")),
    ]);
    main_loop(connection, initialization_params, fs)?;
    io_threads.join()?;

    Ok(())
}

/// Run the main loop.
fn main_loop<FS: FileSystem>(
    connection: Connection,
    params: serde_json::Value,
    fs: FS,
) -> anyhow::Result<()> {
    let _params: lsp_types::InitializeParams = serde_json::from_value(params).unwrap();
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }

                use handlers::request as handlers;
                use lsp_types::request as reqs;

                let state = MessageState::Unhandled(req)
                    .handle::<reqs::HoverRequest, _>(handlers::handle_hover_builder(&fs))?
                    .handle::<reqs::CodeActionRequest, _>(handlers::handle_code_action)?
                    .handle::<reqs::ExecuteCommand, _>(handlers::handle_execute_command)?;

                if let MessageState::Handled(response) = state {
                    connection.sender.send(Message::Response(response))?;
                } else if let MessageState::Unhandled(req) = state {
                    eprintln!("Unhandled request: {req:?}");
                }
            }
            Message::Notification(not) => {
                use handlers::notification as handlers;
                use lsp_types::notification as nots;

                let state = MessageState::Unhandled(not)
                    .handle::<nots::DidOpenTextDocument, _>(
                        handlers::did_open_text_document_builder(&fs),
                    )?
                    .handle::<nots::DidChangeTextDocument, _>(
                        handlers::did_change_text_document_builder(&fs),
                    )?;

                if let MessageState::Unhandled(not) = state {
                    eprintln!("Unhandled notification: {not:?}");
                }
            }
            Message::Response(_resp) => (),
        }
    }
    Ok(())
}
