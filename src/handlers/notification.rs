//! Handlers for notifications.

use anyhow::Result;

/// Create handler for did open text document notification.
pub(crate) fn did_open_text_document_builder<FS: vfs::FileSystem>(
    fs: &FS,
) -> impl FnOnce(lsp_types::DidOpenTextDocumentParams) -> Result<()> + '_ {
    |params| {
        let path = params.text_document.uri.path();
        let contents = params.text_document.text;
        let mut file = fs.create_file(path)?;

        write!(file, "{contents}")?;

        Ok(())
    }
}

/// Create handler for did change text document notification.
pub(crate) fn did_change_text_document_builder<FS: vfs::FileSystem>(
    fs: &FS,
) -> impl FnOnce(lsp_types::DidChangeTextDocumentParams) -> Result<()> + '_ {
    |params| {
        let path = params.text_document.uri.path();
        for change in params.content_changes {
            let contents = change.text;
            let mut file = fs.create_file(path)?;

            write!(file, "{contents}")?;
        }

        Ok(())
    }
}
