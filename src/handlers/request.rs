//! Handlers for requests.

use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Local, NaiveDateTime, Utc};

/// Create handler for hover request.
pub(crate) fn handle_hover_builder<FS: vfs::FileSystem>(
    fs: &FS,
) -> impl FnOnce(lsp_types::HoverParams) -> Result<Option<lsp_types::Hover>> + '_ {
    |params| {
        let Ok(blame_text) = get_blame_text(fs, &params) else {
            return Ok(None);
        };
        use lsp_types::{MarkupContent, MarkupKind::Markdown};
        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Markup(MarkupContent {
                kind: Markdown,
                value: blame_text,
            }),
            range: None,
        };

        Ok(Some(hover))
    }
}

/// Get formatted blame text for given `params`.
fn get_blame_text<FS: vfs::FileSystem>(
    fs: &FS,
    params: &lsp_types::HoverParams,
) -> anyhow::Result<String> {
    let path = params
        .text_document_position_params
        .text_document
        .uri
        .path();
    let repository = git2::Repository::discover(path)?;
    let mut buffer = String::new();

    fs.open_file(path)?.read_to_string(&mut buffer)?;

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

/// Look up hunk from `blame` for given `line`.
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

pub(crate) enum Command {
    StageFile,
    UnstageFile,
}

impl Command {
    const STAGE_FILE_COMMAND: &'static str = "stage_file";
    const UNSTAGE_FILE_COMMAND: &'static str = "unstage_file";

    pub(crate) fn from_str(str: &str) -> Result<Self> {
        if Self::STAGE_FILE_COMMAND == str {
            Ok(Command::StageFile)
        } else if Self::UNSTAGE_FILE_COMMAND == str {
            Ok(Command::UnstageFile)
        } else {
            Err(anyhow::Error::msg("Unknown command"))
        }
    }

    pub(crate) fn to_str(&self) -> &str {
        match self {
            Command::StageFile => Self::STAGE_FILE_COMMAND,
            Command::UnstageFile => Self::UNSTAGE_FILE_COMMAND,
        }
    }
}

/// Handler for code action request.
pub(crate) fn handle_code_action(
    params: lsp_types::CodeActionParams,
) -> Result<Option<Vec<lsp_types::CodeActionOrCommand>>> {
    let mut res = Vec::<lsp_types::CodeActionOrCommand>::new();
    let path = params.text_document.uri.path();
    let Ok(repository) = git2::Repository::discover(path) else {
        return Ok(Some(res));
    };
    let base = repository.workdir().unwrap().to_str().unwrap();
    let rel_path = Path::new(path).strip_prefix(base)?;
    let Ok(status) = repository.status_file(rel_path) else {
        return Ok(Some(res));
    };
    use git2::Status;
    let is_index = status.intersects(
        Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE,
    );
    let is_workdir = status.intersects(
        Status::WT_NEW
            | Status::WT_MODIFIED
            | Status::WT_DELETED
            | Status::WT_TYPECHANGE
            | Status::WT_RENAMED,
    );
    let arguments = Some(vec![serde_json::to_value(path).unwrap()]);

    if is_workdir {
        res.push(lsp_types::CodeActionOrCommand::Command(
            lsp_types::Command {
                title: "Stage file".to_string(),
                command: Command::StageFile.to_str().to_string(),
                arguments,
            },
        ));
    } else if is_index {
        res.push(lsp_types::CodeActionOrCommand::Command(
            lsp_types::Command {
                title: "Unstage file".to_string(),
                command: Command::UnstageFile.to_str().to_string(),
                arguments,
            },
        ));
    }

    Ok(Some(res))
}

/// Handler for executing commands.
pub(crate) fn handle_execute_command(
    params: lsp_types::ExecuteCommandParams,
) -> Result<Option<serde_json::Value>> {
    let command = Command::from_str(&params.command)?;
    let path = params.arguments.first().unwrap().as_str().unwrap();
    let Ok(repository) = git2::Repository::discover(path) else {
        return Ok(None);
    };
    let mut index = repository.index()?;
    let base = repository.workdir().unwrap().to_str().unwrap();
    let rel_path = Path::new(path).strip_prefix(base)?;

    match command {
        Command::StageFile => {
            index.add_path(rel_path)?;
        }
        Command::UnstageFile => {
            // TODO: Completely removes the file from the index but should unstage
            index.remove_path(rel_path)?
        }
    }

    index.write()?;

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Url;

    #[test]
    fn it_works() {
        let fs = vfs::PhysicalFS::new("/");
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

        let result = get_blame_text(&fs, &hover_params);

        assert!(result.is_ok());
    }
}
