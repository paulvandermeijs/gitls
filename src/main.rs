mod message_state;
mod handlers {
    pub(crate) mod notification;
    pub(crate) mod request;
}

use lsp_server::{Connection, Message};
use lsp_types::{HoverProviderCapability, InitializeParams, ServerCapabilities};

use crate::message_state::MessageState;

fn main() -> anyhow::Result<()> {
    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let hover_provider = Some(HoverProviderCapability::Simple(true));
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        hover_provider,
        ..Default::default()
    })
    .unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    main_loop(connection, initialization_params)?;
    io_threads.join()?;

    Ok(())
}

fn main_loop(connection: Connection, params: serde_json::Value) -> anyhow::Result<()> {
    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }

                use handlers::request as handlers;
                use lsp_types::request as reqs;

                let state = MessageState::Unhandled(req)
                    .handle::<reqs::HoverRequest, _>(handlers::handle_hover)?;

                if let MessageState::Handled(Some(response)) = state {
                    connection.sender.send(Message::Response(response))?;
                } else if let MessageState::Unhandled(req) = state {
                    eprintln!("Unhandled request: {req:?}");
                }
            }
            Message::Notification(not) => {
                use handlers::notification as handlers;
                use lsp_types::notification as nots;

                let state = MessageState::Unhandled(not)
                    .handle::<nots::DidOpenTextDocument, _>(handlers::did_open_text_document)?;

                if let MessageState::Unhandled(not) = state {
                    eprintln!("Unhandled notification: {not:?}");
                }
            }
            Message::Response(_resp) => (),
        }
    }
    Ok(())
}
