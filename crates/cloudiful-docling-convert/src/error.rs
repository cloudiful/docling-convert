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
                    write!(f, "{}", src)?;
                    let mut curr = src.source();
                    while let Some(cause) = curr {
                        write!(f, " caused by: {}", cause)?;
                        curr = cause.source();
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

impl From<lopdf::Error> for PdfConvertError {
    fn from(err: lopdf::Error) -> Self {
        PdfConvertError::ParseError {
            target: "PDF".to_string(),
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
}
