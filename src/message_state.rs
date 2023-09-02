use anyhow::Result;

pub(crate) enum MessageState<I, O> {
    Unhandled(I),
    Handled(O),
}

impl MessageState<lsp_server::Notification, ()> {
    pub fn handle<N, F>(self, handler: F) -> Result<Self>
    where
        N: lsp_types::notification::Notification,
        N::Params: serde::de::DeserializeOwned,
        F: FnOnce(N::Params) -> Result<()>,
    {
        match self {
            Self::Unhandled(not) => {
                if let Ok(params) = not.clone().extract(N::METHOD) {
                    handler(params)?;
                    return Ok(Self::Handled(()));
                }

                Ok(Self::Unhandled(not))
            }
            Self::Handled(_) => Ok(self),
        }
    }
}

impl MessageState<lsp_server::Request, Option<lsp_server::Response>> {
    pub fn handle<R, F>(self, handler: F) -> Result<Self>
    where
        R: lsp_types::request::Request,
        R::Params: serde::de::DeserializeOwned,
        F: FnOnce(lsp_server::RequestId, R::Params) -> Result<Option<lsp_server::Response>>,
    {
        match self {
            Self::Unhandled(req) => {
                if let Ok((id, params)) = req.clone().extract(R::METHOD) {
                    let result = handler(id, params)?;
                    return Ok(Self::Handled(result));
                }

                Ok(Self::Unhandled(req))
            }
            Self::Handled(_) => Ok(self),
        }
    }
}
