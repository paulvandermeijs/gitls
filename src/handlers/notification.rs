use anyhow::Result;

pub(crate) fn did_open_text_document(params: lsp_types::DidOpenTextDocumentParams) -> Result<()> {
    eprintln!("Did open text document: {params:?}");
    Ok(())
}
