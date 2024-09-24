//! # Message State
//!
//! Message state is used to represent handled or unhandled notifications and
//! requests.

use anyhow::Result;

pub(crate) enum MessageState<I, O> {
    /// Unhandled state with input `I`.
    Unhandled(I),
    /// Handled state with output `O`.
    Handled(O),
}

/// Message state for notifications.
impl MessageState<lsp_server::Notification, ()> {
    /// Apply `handler` if params match notification type `N`.
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

/// Message state for requests.
impl MessageState<lsp_server::Request, lsp_server::Response> {
    /// Apply `handler` if params match request type `R`.
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
