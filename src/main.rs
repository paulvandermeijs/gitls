use std::error::Error;
use std::path::Path;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use lsp_types::{
    request::HoverRequest, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    MarkupContent, ServerCapabilities,
};

use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Note that  we must have our logging only write out to stderr.
    eprintln!("starting generic LSP server");

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

    // Shut down gracefully.
    eprintln!("shutting down server");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    eprintln!("starting example main loop");
    for msg in &connection.receiver {
        eprintln!("got msg: {msg:?}");
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                eprintln!("got request: {req:?}");
                match cast::<HoverRequest>(req) {
                    Ok((id, params)) => {
                        eprintln!("got gotoDefinition request #{id}: {params:?}");
                        if let Ok(blame_text) = get_blame_text(&params) {
                            let hover = Hover {
                                contents: lsp_types::HoverContents::Markup(MarkupContent {
                                    kind: lsp_types::MarkupKind::Markdown,
                                    value: blame_text,
                                }),
                                range: None,
                            };
                            let result = Some(hover);
                            let result = serde_json::to_value(&result).unwrap();
                            let response = Response {
                                id,
                                result: Some(result),
                                error: None,
                            };
                            connection.sender.send(Message::Response(response))?;
                        }
                        continue;
                    }
                    Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
                    Err(ExtractError::MethodMismatch(req)) => req,
                };
            }
            Message::Response(resp) => {
                eprintln!("got response: {resp:?}");
            }
            Message::Notification(not) => {
                eprintln!("got notification: {not:?}");
            }
        }
    }
    Ok(())
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

fn get_blame_text(params: &HoverParams) -> Result<String, String> {
    let repository = git2::Repository::discover(
        params
            .text_document_position_params
            .text_document
            .uri
            .path(),
    )
    .map_err(|e| e.message().to_string())?;

    let base = repository.workdir().unwrap().to_str().unwrap();

    // TODO: Use `git_blame_buffer` to include file changes
    let blame = repository
        .blame_file(
            Path::new(
                params
                    .text_document_position_params
                    .text_document
                    .uri
                    .path(),
            )
            .strip_prefix(base)
            .map_err(|e| e.to_string())?,
            None,
        )
        .map_err(|e| e.message().to_string())?;

    let lineno: usize = params
        .text_document_position_params
        .position
        .line
        .try_into()
        .unwrap();
    let lineno = lineno + 1;

    let line = blame
        .get_line(lineno)
        .ok_or("Failed to get line from blame")?;

    let commit = repository
        .find_commit(line.final_commit_id())
        .map_err(|e| e.message().to_string())?;

    let date_time = DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp_opt(commit.time().seconds(), 0).unwrap(),
        Utc,
    );
    let date_time = date_time.with_timezone(&Local);

    Ok(format!(
        "{} {}: {}",
        commit.author().name().unwrap(),
        date_time,
        commit.message().unwrap()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Url;

    #[test]
    fn it_works() {
        let hover_params = lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: Url::parse(&format!(
                        "file://{}/README.md",
                        std::env::current_dir().unwrap().to_str().unwrap()
                    ))
                    .unwrap(),
                },
                position: lsp_types::Position {
                    line: 1,
                    character: 1,
                },
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let result = get_blame_text(&hover_params);

        assert!(result.is_ok());
    }
}
