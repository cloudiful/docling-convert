use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum PdfConvertError {
    IoError {
        context: String,
        source: std::io::Error,
    },

    ApiError {
        status_code: Option<u16>,
        message: String,
        source: Option<reqwest::Error>,
    },

    ParseError {
        target: String,
        message: String,
    },

    #[allow(dead_code)]
    ValidationError {
        parameter: String,
        reason: String,
    },

    EnvError {
        var_name: String,
        message: String,
    },

    OperationError {
        context: String,
        message: String,
    },
}

impl fmt::Display for PdfConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfConvertError::IoError { context, source } => {
                write!(f, "IO error while {}: {}", context, source)?;
                let mut curr = source.source();
                while let Some(src) = curr {
                    write!(f, " caused by: {}", src)?;
                    curr = src.source();
                }
                Ok(())
            }
            PdfConvertError::ApiError {
                status_code,
                message,
                source,
            } => {
                if let Some(src) = source {
                    // Robust Display with compatibility (issue #176): the
                    // `From<reqwest::Error>` path used to hide the HTTP status
                    // behind the reqwest message, forcing callers to parse
                    // `Display` for `HTTP XXX`. Expose the effective status
                    // when known while keeping the original reqwest message
                    // (and its cause chain) intact so existing log matching
                    // keeps working; callers should prefer `status_code()`.
                    let effective =
                        (*status_code).or_else(|| src.status().map(|status| status.as_u16()));
                    if let Some(code) = effective {
                        write!(f, "HTTP {}: {}", code, src)?;
                    } else {
                        write!(f, "{}", src)?;
                    }
                    let mut curr = src.source();
                    while let Some(cause) = curr {
                        write!(f, " caused by: {}", cause)?;
                        curr = cause.source();
                    }
                    // Preserve a divergent stored message (normally equal to
                    // `src.to_string()` via `From<reqwest::Error>`) without
                    // breaking the `HTTP {code}: {src}` prefix contract.
                    if message != &src.to_string() && !message.is_empty() {
                        write!(f, " ({})", message)?;
                    }
                    Ok(())
                } else if let Some(code) = status_code {
                    write!(f, "HTTP {}: {}", code, message)
                } else {
                    write!(f, "{}", message)
                }
            }
            PdfConvertError::ParseError { target, message } => {
                write!(f, "Failed to parse {}: {}", target, message)
            }
            PdfConvertError::ValidationError { parameter, reason } => {
                write!(f, "Validation error for '{}': {}", parameter, reason)
            }
            PdfConvertError::EnvError { var_name, message } => {
                write!(
                    f,
                    "Environment variable error for '{}': {}",
                    var_name, message
                )
            }
            PdfConvertError::OperationError { context, message } => {
                write!(f, "{}: {}", context, message)
            }
        }
    }
}

impl std::error::Error for PdfConvertError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PdfConvertError::IoError { source, .. } => Some(source),
            PdfConvertError::ApiError { source, .. } => source
                .as_ref()
                .map(|e| e as &(dyn std::error::Error + 'static)),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, PdfConvertError>;

impl From<std::io::Error> for PdfConvertError {
    fn from(err: std::io::Error) -> Self {
        PdfConvertError::IoError {
            context: "performing file operation".to_string(),
            source: err,
        }
    }
}

impl From<reqwest::Error> for PdfConvertError {
    fn from(err: reqwest::Error) -> Self {
        let status_code = err.status().map(|s| s.as_u16());
        let message = err.to_string();

        PdfConvertError::ApiError {
            status_code,
            message,
            source: Some(err),
        }
    }
}

impl From<serde_json::Error> for PdfConvertError {
    fn from(err: serde_json::Error) -> Self {
        PdfConvertError::ParseError {
            target: "JSON".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<std::env::VarError> for PdfConvertError {
    fn from(err: std::env::VarError) -> Self {
        match err {
            std::env::VarError::NotPresent => PdfConvertError::EnvError {
                var_name: "unknown".to_string(),
                message: "Environment variable not set".to_string(),
            },
            std::env::VarError::NotUnicode(_) => PdfConvertError::EnvError {
                var_name: "unknown".to_string(),
                message: "Environment variable contains invalid Unicode".to_string(),
            },
        }
    }
}

impl PdfConvertError {
    pub fn io_error(context: impl Into<String>, source: std::io::Error) -> Self {
        PdfConvertError::IoError {
            context: context.into(),
            source,
        }
    }

    pub fn api_error(status_code: Option<u16>, message: impl Into<String>) -> Self {
        PdfConvertError::ApiError {
            status_code,
            message: message.into(),
            source: None,
        }
    }

    pub fn parse_error(target: impl Into<String>, message: impl Into<String>) -> Self {
        PdfConvertError::ParseError {
            target: target.into(),
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub fn validation_error(parameter: impl Into<String>, reason: impl Into<String>) -> Self {
        PdfConvertError::ValidationError {
            parameter: parameter.into(),
            reason: reason.into(),
        }
    }

    pub fn env_error(var_name: impl Into<String>, message: impl Into<String>) -> Self {
        PdfConvertError::EnvError {
            var_name: var_name.into(),
            message: message.into(),
        }
    }

    pub fn operation_error(context: impl Into<String>, message: impl Into<String>) -> Self {
        PdfConvertError::OperationError {
            context: context.into(),
            message: message.into(),
        }
    }

    pub fn api_task_failed(status: impl Into<String>, details: impl Into<String>) -> Self {
        PdfConvertError::ApiError {
            status_code: None,
            message: format!(
                "Task failed - Status: {}, Details: {}",
                status.into(),
                details.into()
            ),
            source: None,
        }
    }

    /// Structured HTTP status for [`PdfConvertError::ApiError`] (issue #176).
    ///
    /// Prefers the stored `status_code` (populated by `api_error`,
    /// `handle_response`, and `From<reqwest::Error>`), then falls back to the
    /// wrapped `reqwest::Error::status()`. Returns `None` for non-API variants
    /// and for transport errors without an HTTP status (DNS, connect,
    /// timeout). UUID hex fragments such as `401de82e` never yield a status;
    /// only a real HTTP status counts.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            PdfConvertError::ApiError {
                status_code: Some(code),
                ..
            } => Some(*code),
            PdfConvertError::ApiError {
                source: Some(source),
                ..
            } => source.status().map(|status| status.as_u16()),
            PdfConvertError::ApiError { status_code, .. } => *status_code,
            _ => None,
        }
    }

    /// Borrowed message for [`PdfConvertError::ApiError`], if present.
    ///
    /// Lets callers read the human-readable detail without parsing
    /// `Display`; returns `None` for non-API variants.
    pub fn api_message(&self) -> Option<&str> {
        match self {
            PdfConvertError::ApiError { message, .. } => Some(message.as_str()),
            _ => None,
        }
    }

    /// Typed `reqwest` cause for [`PdfConvertError::ApiError`], if present.
    ///
    /// Useful when callers need more than the status (e.g. timeout vs.
    /// status errors) without downcasting via [`std::error::Error::source`].
    pub fn reqwest_source(&self) -> Option<&reqwest::Error> {
        match self {
            PdfConvertError::ApiError { source, .. } => source.as_ref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_status_code_accessor_matches_display_for_auth_and_server_errors() {
        // True HTTP statuses must round-trip through the structured accessor
        // and stay visible in `Display` (issue #176).
        for status in [401u16, 403, 404, 429, 500, 502, 503] {
            let message = format!("upstream failed with test status {status}");
            let error = PdfConvertError::api_error(Some(status), message.clone());
            assert_eq!(
                error.status_code(),
                Some(status),
                "accessor must return structured status {status}"
            );
            assert_eq!(error.api_message(), Some(message.as_str()));
            assert!(error.reqwest_source().is_none());
            let display = error.to_string();
            assert!(
                display.contains(&format!("HTTP {status}")),
                "Display must expose HTTP {status}: {display}"
            );
            assert!(
                display.contains(&message),
                "Display must keep message: {display}"
            );
        }
    }

    #[test]
    fn uuid_containing_401_has_no_status_code_and_no_http_prefix() {
        // UUID hex fragments such as `401de82e` must not be mistaken for a
        // standalone HTTP 401/403, and a `:5001` port must not count as 5xx.
        // Both the accessor and `Display` must stay consistent (issue #176).
        let uuid_message =
            "failed to poll docling task 401de82e-e717-4f6a-9c2a-9b1a2c3d4e5f: dns error";
        let error = PdfConvertError::api_error(None, uuid_message);
        assert_eq!(error.status_code(), None);
        assert_eq!(error.api_message(), Some(uuid_message));
        let display = error.to_string();
        assert_eq!(display, uuid_message);
        assert!(
            !display.contains("HTTP 401") && !display.contains("HTTP 403"),
            "UUID Display must not fabricate an HTTP status: {display}"
        );

        let port_message = "http://192.168.67.31:5001/v1/status/poll unreachable";
        let port_error = PdfConvertError::api_error(None, port_message);
        assert_eq!(port_error.status_code(), None);
        assert!(!port_error.to_string().contains("HTTP 5"));

        // Non-API variants never carry a status either.
        let operation =
            PdfConvertError::operation_error("poll 401de82e task", "dns error for task");
        assert_eq!(operation.status_code(), None);
        assert_eq!(operation.api_message(), None);
        assert!(operation.reqwest_source().is_none());
        assert!(!operation.to_string().contains("HTTP 401"));
    }

    async fn reqwest_error_with_status(status: u16) -> reqwest::Error {
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let reason = match status {
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    503 => "Service Unavailable",
                    _ => "Error",
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        let url = format!("http://{addr}/");
        let error = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .expect("send")
            .error_for_status()
            .expect_err("expected HTTP error status");
        let _ = server.await;
        assert_eq!(error.status().map(|status| status.as_u16()), Some(status));
        error
    }

    #[tokio::test]
    async fn from_reqwest_preserves_status_and_display_exposes_http_code() {
        // The `From<reqwest::Error>` path must retain the HTTP status in the
        // structured accessor even though `Display` previously hid it behind
        // the reqwest message (issue #176).
        for status in [401u16, 503] {
            let reqwest_error = reqwest_error_with_status(status).await;
            let error = PdfConvertError::from(reqwest_error);
            assert_eq!(
                error.status_code(),
                Some(status),
                "From<reqwest> must preserve HTTP {status}"
            );
            assert!(error.reqwest_source().is_some());
            assert_eq!(
                error
                    .reqwest_source()
                    .and_then(|source| source.status().map(|status| status.as_u16())),
                Some(status)
            );
            let display = error.to_string();
            assert!(
                display.contains(&format!("HTTP {status}")),
                "Display with source must still expose HTTP {status}: {display}"
            );
        }
    }
}
