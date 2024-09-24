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

impl MessageState<lsp_server::Request, lsp_server::Response> {
    pub fn handle<R, F>(self, handler: F) -> Result<Self>
    where
        R: lsp_types::request::Request,
        R::Params: serde::de::DeserializeOwned,
        F: FnOnce(R::Params) -> Result<R::Result>,
    {
        match self {
            Self::Unhandled(req) => {
                if let Ok((id, params)) = req.clone().extract(R::METHOD) {
                    let result = handler(params)?;
                    let result = serde_json::to_value(&result).unwrap();
                    let response = lsp_server::Response {
                        id,
                        result: Some(result),
                        error: None,
                    };

                    return Ok(Self::Handled(response));
                }

                Ok(Self::Unhandled(req))
            }
            Self::Handled(_) => Ok(self),
        }
    }
}
