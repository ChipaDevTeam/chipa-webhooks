uniffi::setup_scaffolding!();

mod config;
mod error;
mod runtime;

mod client;
pub use client::WebhookClient;

pub use error::WebhookFfiError;

#[cfg(feature = "c")]
mod ffi_c;
