use git2::Commit;

use super::date_time::format_timestamp;

pub(crate) fn format_blame_text(commit: Commit) -> String {
    let Some(message) = commit.message() else {
        return "No message".to_string();
    };

    let message = message.trim();
    let (subject, body) = match message.find("\n\n") {
        Some(_) => message.split_once("\n\n").unwrap(),
        None => (message, ""),
    };
    let author = commit.author();
    let author = author.name().unwrap();
    let timestamp = format_timestamp(commit.time().seconds() as u64);
    let commit_id = &commit.id().to_string()[..8];
    let blame_text = format!("**{subject}**\n*{author} • {timestamp} • {commit_id}*\n\n{body}");
    let blame_text = blame_text.trim().to_string();

    blame_text
}
