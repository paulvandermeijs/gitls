use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use lsp_server::{RequestId, Response};

pub(crate) fn handle_hover(
    id: RequestId,
    params: lsp_types::HoverParams,
) -> Result<Option<Response>> {
    let blame_text = get_blame_text(&params)?;
    use lsp_types::{MarkupContent, MarkupKind::Markdown};
    let hover = lsp_types::Hover {
        contents: lsp_types::HoverContents::Markup(MarkupContent {
            kind: Markdown,

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
    Ok(Some(response))
}

fn get_blame_text(params: &lsp_types::HoverParams) -> anyhow::Result<String> {
    let path = params
        .text_document_position_params
        .text_document
        .uri
        .path();
    let repository = git2::Repository::discover(path)?;
    let buffer = std::fs::read_to_string(path).unwrap();
    let base = repository.workdir().unwrap().to_str().unwrap();
    let blame = repository.blame_file(Path::new(path).strip_prefix(base)?, None)?;
    let blame = blame.blame_buffer(buffer.as_bytes()).unwrap();

    let lineno = params.text_document_position_params.position.line;
    let lineno = lineno + 1;

    let line = get_hunk_for_line(&blame, lineno.try_into().unwrap())
        .map_err(|_| anyhow::Error::msg("Failed to get line from blame"))?;

    if line.final_commit_id().is_zero() {
        return Ok("Uncommitted changes".to_string());
    }

    let commit = repository.find_commit(line.final_commit_id())?;

    let date_time = DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp_opt(commit.time().seconds(), 0).unwrap(),
        Utc,
    );
    let date_time = date_time.with_timezone(&Local);

    Ok(format!(
        "{} {}: {}",
        commit.author().name().unwrap(),
        date_time,
        commit.message().unwrap(),
    ))
}

fn get_hunk_for_line<'a>(
    blame: &'a git2::Blame<'a>,
    line: usize,
) -> Result<git2::BlameHunk<'a>, Box<dyn std::error::Error>> {
    let mut current_line = 1;
    for hunk in blame.iter() {
        current_line += hunk.lines_in_hunk();

        if line < current_line {
            return Ok(hunk);
        }
    }

    Err("Line not found".into())
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
