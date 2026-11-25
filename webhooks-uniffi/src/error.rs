use thiserror::Error;

#[derive(Debug, Error, uniffi::Error)]
pub enum WebhookFfiError {
    #[error("Config error: {message}")]
    Config { message: String },

    #[error("Template error: {message}")]
    Template { message: String },

    #[error("Dispatch error: {message}")]
    Dispatch { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

pub(crate) fn format_report(err: impl std::error::Error + 'static) -> String {
    use rootcause::IntoReport;
    let report = err.into_report();
    format!("{report}")
}

pub(crate) fn into_config_error(
    err: impl std::error::Error + 'static,
    context: &str,
) -> WebhookFfiError {
    WebhookFfiError::Config {
        message: format!("{context}\n\nCaused by:\n{}", format_report(err)),
    }
}

pub(crate) fn into_dispatch_error(err: chipa_webhooks::WebhookError) -> WebhookFfiError {
    WebhookFfiError::Dispatch {
        message: format_report(err),
    }
}

pub(crate) fn into_template_error(err: chipa_webhooks::WebhookError) -> WebhookFfiError {
    WebhookFfiError::Template {
        message: format_report(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_format_report() {
        let err = io::Error::new(io::ErrorKind::Other, "test error");
        let report = format_report(err);
        assert!(report.contains("test error"));
    }

    #[test]
    fn test_into_config_error() {
        let err = io::Error::new(io::ErrorKind::Other, "inner error");
        let config_err = into_config_error(err, "context message");
        match config_err {
            WebhookFfiError::Config { message } => {
                assert!(message.contains("context message"));
                assert!(message.contains("inner error"));
            }
            _ => panic!("Expected Config error"),
        }
    }

    #[test]
    fn test_error_display() {
        let err = WebhookFfiError::Internal {
            message: "test".to_string(),
        };
        assert_eq!(err.to_string(), "Internal error: test");
    }

    #[test]
    fn test_dispatch_error_display() {
        let err = WebhookFfiError::Dispatch {
            message: "dispatch failure".to_string(),
        };
        assert_eq!(err.to_string(), "Dispatch error: dispatch failure");
    }

    #[test]
    fn test_template_error_display() {
        let err = WebhookFfiError::Template {
            message: "template failure".to_string(),
        };
        assert_eq!(err.to_string(), "Template error: template failure");
    }

    #[test]
    fn test_config_error_display() {
        let err = WebhookFfiError::Config {
            message: "config failure".to_string(),
        };
        assert_eq!(err.to_string(), "Config error: config failure");
    }
}
