use std::sync::{Arc, RwLock};

use chipa_webhooks::WebhookDispatcher;

use crate::config::ClientConfig;
use crate::error::{WebhookFfiError, into_config_error, into_dispatch_error, into_template_error};
use crate::runtime::block_on;

#[uniffi::export(callback_interface)]
pub trait MatcherCallback: Send + Sync {
    fn match_template(&self, payload_json: String) -> Option<String>;
}

#[derive(uniffi::Object)]
pub struct WebhookClient {
    dispatcher: WebhookDispatcher,
    matchers: RwLock<Vec<Box<dyn MatcherCallback>>>,
}

#[uniffi::export]
impl WebhookClient {
    /// Create a new `WebhookClient` from a JSON config string.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient = WebhookClient(json.dumps(config))\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nval client = WebhookClient(configJson)\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\nlet client = try WebhookClient(configJson: configJson)\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nWebhookClient client = new WebhookClient(configJson);\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nauto client = WebhookClient(config_json);\n```"
    )]
    #[uniffi::constructor]
    pub fn new(config_json: String) -> Result<Arc<Self>, WebhookFfiError> {
        let config = ClientConfig::parse(&config_json)?;
        let capacity = config.capacity.unwrap_or(1024);

        let mut builder = WebhookDispatcher::builder()
            .capacity(capacity)
            .on_error(|e| {
                tracing::error!(error = %e, "webhook dispatch error");
            });

        for dest_cfg in config.destinations {
            builder = builder.destination(dest_cfg.into_destination());
        }

        if let Some(templates) = config.templates {
            for (name, template) in templates {
                builder = builder.template(name, template);
            }
        }

        let dispatcher = block_on(async move { builder.build().map_err(into_template_error) })?;

        Ok(Arc::new(Self {
            dispatcher,
            matchers: RwLock::new(Vec::new()),
        }))
    }

    /// Send a webhook using the named template and a raw JSON payload string.
    ///
    /// `payload_json` must be a valid JSON object string.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient.send_sync('alerts', json.dumps(event))\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.sendSync(\"alerts\", gson.toJson(event))\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\ntry client.sendSync(templateName: \"alerts\", payloadJson: encoded)\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.sendSync(\"alerts\", payload);\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->sendSync(\"alerts\", payload);\n```"
    )]
    pub fn send_sync(
        &self,
        template_name: String,
        payload_json: String,
    ) -> Result<(), WebhookFfiError> {
        block_on(self.send_inner(&template_name, &payload_json))
    }

    /// Resolve the template via registered matchers (falling back to "default") and send.
    ///
    /// `payload_json` must be a valid JSON object string.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient.dispatch_sync(json.dumps(event))\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.dispatchSync(gson.toJson(event))\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\ntry client.dispatchSync(payloadJson: encoded)\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.dispatchSync(payload);\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->dispatchSync(payload);\n```"
    )]
    pub fn dispatch_sync(&self, payload_json: String) -> Result<(), WebhookFfiError> {
        let template_name = self.resolve_template(&payload_json);
        block_on(self.send_inner(&template_name, &payload_json))
    }

    /// Register a matcher callback. The first matcher returning `Some` wins.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient.register_matcher(AlertMatcher())\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.registerMatcher(AlertMatcher())\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\nclient.registerMatcher(matcher: AlertMatcher())\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.registerMatcher(new AlertMatcher());\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->registerMatcher(std::make_shared<AlertMatcher>());\n```"
    )]
    pub fn register_matcher(&self, matcher: Box<dyn MatcherCallback>) {
        self.matchers
            .write()
            .expect("matchers lock poisoned")
            .push(matcher);
    }

    /// Register a new template at runtime.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient.register_template('alerts', '🚨 {{message}}')\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.registerTemplate(\"alerts\", \"🚨 {{message}}\")\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\ntry client.registerTemplate(name: \"alerts\", template: \"🚨 {{message}}\")\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.registerTemplate(\"alerts\", \"🚨 {{message}}\");\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->registerTemplate(\"alerts\", \"🚨 {{message}}\");\n```"
    )]
    pub fn register_template(&self, name: String, template: String) -> Result<(), WebhookFfiError> {
        self.dispatcher
            .register_template(&name, &template)
            .map_err(into_template_error)
    }

    /// Overwrite an existing template at runtime.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient.update_template('alerts', '⚠️ {{message}}')\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.updateTemplate(\"alerts\", \"⚠️ {{message}}\")\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\ntry client.updateTemplate(name: \"alerts\", template: \"⚠️ {{message}}\")\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.updateTemplate(\"alerts\", \"⚠️ {{message}}\");\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->updateTemplate(\"alerts\", \"⚠️ {{message}}\");\n```"
    )]
    pub fn update_template(&self, name: String, template: String) -> Result<(), WebhookFfiError> {
        self.dispatcher
            .update_template(&name, &template)
            .map_err(into_template_error)
    }

    /// Remove a template at runtime.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient.remove_template('alerts')\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.removeTemplate(\"alerts\")\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\nclient.removeTemplate(name: \"alerts\")\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.removeTemplate(\"alerts\");\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->removeTemplate(\"alerts\");\n```"
    )]
    pub fn remove_template(&self, name: String) {
        self.dispatcher.remove_template(&name);
    }

    /// Flush queued jobs and shut down the dispatcher.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nclient.shutdown_sync()\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.shutdownSync()\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\nclient.shutdownSync()\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.shutdownSync();\n```"
    )]
    #[cfg_attr(feature = "cpp", doc = "### C++\n```cpp\nclient->shutdownSync();\n```")]
    pub fn shutdown_sync(&self) {
        block_on(self.flush());
    }
}

#[cfg(feature = "async")]
#[uniffi::export]
impl WebhookClient {
    /// Send a webhook using the named template and a raw JSON payload string (async).
    ///
    /// `payload_json` must be a valid JSON object string.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nawait client.send('alerts', json.dumps(event))\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.send(\"alerts\", gson.toJson(event))\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\ntry await client.send(templateName: \"alerts\", payloadJson: encoded)\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.send(\"alerts\", payload).get();\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->send(\"alerts\", payload);\n```"
    )]
    pub async fn send(
        &self,
        template_name: String,
        payload_json: String,
    ) -> Result<(), WebhookFfiError> {
        self.send_inner(&template_name, &payload_json).await
    }

    /// Matcher-based dispatch (async).
    ///
    /// `payload_json` must be a valid JSON object string.
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nawait client.dispatch(json.dumps(event))\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.dispatch(gson.toJson(event))\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\ntry await client.dispatch(payloadJson: encoded)\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.dispatch(payload).get();\n```"
    )]
    #[cfg_attr(
        feature = "cpp",
        doc = "### C++\n```cpp\nclient->dispatch(payload);\n```"
    )]
    pub async fn dispatch(&self, payload_json: String) -> Result<(), WebhookFfiError> {
        let template_name = self.resolve_template(&payload_json);
        self.send_inner(&template_name, &payload_json).await
    }

    /// Graceful shutdown (async).
    #[cfg_attr(
        feature = "python",
        doc = "### Python\n```python\nawait client.shutdown()\n```"
    )]
    #[cfg_attr(
        feature = "kotlin",
        doc = "### Kotlin\n```kotlin\nclient.shutdown()\n```"
    )]
    #[cfg_attr(
        feature = "swift",
        doc = "### Swift\n```swift\nawait client.shutdown()\n```"
    )]
    #[cfg_attr(
        feature = "java",
        doc = "### Java\n```java\nclient.shutdown().get();\n```"
    )]
    #[cfg_attr(feature = "cpp", doc = "### C++\n```cpp\nclient->shutdown();\n```")]
    pub async fn shutdown(&self) {
        self.flush().await;
    }
}

impl WebhookClient {
    /// Walk registered matchers in order; return the first `Some` result or `"default"`.
    fn resolve_template(&self, payload_json: &str) -> String {
        let matchers = self.matchers.read().expect("matchers lock poisoned");
        for matcher in matchers.iter() {
            if let Some(name) = matcher.match_template(payload_json.to_owned()) {
                return name;
            }
        }
        "default".to_owned()
    }

    async fn send_inner(
        &self,
        template_name: &str,
        payload_json: &str,
    ) -> Result<(), WebhookFfiError> {
        let value: serde_json::Value = serde_json::from_str(payload_json)
            .map_err(|e| into_config_error(e, "payload_json is not valid JSON"))?;

        self.dispatcher
            .send_with_template(template_name, &value)
            .await
            .map_err(into_dispatch_error)
    }

    /// Flush all queued dispatch jobs. Used by `ffi_c` and `shutdown_sync`.
    pub(crate) async fn flush(&self) {
        self.dispatcher.flush().await.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"{
        "destinations": [
            {
                "name": "test-discord",
                "credentials": {
                    "type": "discord",
                    "webhook_url": "https://discord.com/api/webhooks/000/fake"
                }
            }
        ],
        "templates": {
            "default": "{{event}}: {{message}}",
            "alerts":  "[ALERT] {{message}}"
        }
    }"#;

    fn make_client() -> Arc<WebhookClient> {
        WebhookClient::new(VALID_CONFIG.to_owned()).expect("valid config must build a client")
    }

    // ── resolve_template ──────────────────────────────────────────────────────

    #[test]
    fn no_matchers_falls_back_to_default() {
        let client = make_client();
        let name = client.resolve_template(r#"{"event":"tick"}"#);
        assert_eq!(name, "default");
    }

    #[test]
    fn single_matcher_returns_some_wins() {
        let client = make_client();

        struct AlertMatcher;
        impl MatcherCallback for AlertMatcher {
            fn match_template(&self, payload_json: String) -> Option<String> {
                if payload_json.contains(r#""alert""#) {
                    Some("alerts".to_owned())
                } else {
                    None
                }
            }
        }

        client.register_matcher(Box::new(AlertMatcher));

        assert_eq!(
            client.resolve_template(r#"{"level":"alert","message":"down"}"#),
            "alerts"
        );
        assert_eq!(
            client.resolve_template(r#"{"level":"info","message":"ok"}"#),
            "default"
        );
    }

    #[test]
    fn first_matcher_wins_second_is_not_called() {
        let client = make_client();

        struct FirstMatcher;
        impl MatcherCallback for FirstMatcher {
            fn match_template(&self, _: String) -> Option<String> {
                Some("alerts".to_owned())
            }
        }

        // This would return a different name, but should never be reached.
        struct SecondMatcher;
        impl MatcherCallback for SecondMatcher {
            fn match_template(&self, _: String) -> Option<String> {
                panic!("second matcher must not be called when first returns Some");
            }
        }

        client.register_matcher(Box::new(FirstMatcher));
        client.register_matcher(Box::new(SecondMatcher));

        let name = client.resolve_template(r#"{"event":"anything"}"#);
        assert_eq!(name, "alerts");
    }

    #[test]
    fn first_matcher_none_chains_to_second() {
        let client = make_client();

        struct PassMatcher;
        impl MatcherCallback for PassMatcher {
            fn match_template(&self, _: String) -> Option<String> {
                None
            }
        }

        struct AlertMatcher;
        impl MatcherCallback for AlertMatcher {
            fn match_template(&self, payload_json: String) -> Option<String> {
                if payload_json.contains(r#""alert""#) {
                    Some("alerts".to_owned())
                } else {
                    None
                }
            }
        }

        client.register_matcher(Box::new(PassMatcher));
        client.register_matcher(Box::new(AlertMatcher));

        assert_eq!(client.resolve_template(r#"{"level":"alert"}"#), "alerts");
        assert_eq!(client.resolve_template(r#"{"level":"info"}"#), "default");
    }

    #[test]
    fn all_matchers_return_none_falls_back_to_default() {
        let client = make_client();

        struct NullMatcher;
        impl MatcherCallback for NullMatcher {
            fn match_template(&self, _: String) -> Option<String> {
                None
            }
        }

        client.register_matcher(Box::new(NullMatcher));
        client.register_matcher(Box::new(NullMatcher));
        client.register_matcher(Box::new(NullMatcher));

        assert_eq!(client.resolve_template(r#"{"event":"x"}"#), "default");
    }

    // ── send_sync / dispatch_sync error paths ─────────────────────────────────

    #[test]
    fn send_sync_invalid_payload_json_returns_config_error() {
        let client = make_client();
        let err = client
            .send_sync("default".to_owned(), "not json {{{".to_owned())
            .unwrap_err();
        assert!(
            matches!(err, WebhookFfiError::Config { .. }),
            "expected Config error, got {err:?}"
        );
    }

    #[test]
    fn dispatch_sync_invalid_payload_json_returns_config_error() {
        let client = make_client();
        let err = client.dispatch_sync("not json".to_owned()).unwrap_err();
        assert!(
            matches!(err, WebhookFfiError::Config { .. }),
            "expected Config error, got {err:?}"
        );
    }

    // ── register_matcher ──────────────────────────────────────────────────────

    #[test]
    fn matchers_accumulate_in_order() {
        let client = make_client();

        struct NamedMatcher(&'static str);
        impl MatcherCallback for NamedMatcher {
            fn match_template(&self, _: String) -> Option<String> {
                Some(self.0.to_owned())
            }
        }

        client.register_matcher(Box::new(NamedMatcher("first")));
        client.register_matcher(Box::new(NamedMatcher("second")));

        let count = client.matchers.read().unwrap().len();
        assert_eq!(count, 2);

        // First registered still wins.
        assert_eq!(client.resolve_template("{}"), "first");
    }

    // ── shutdown_sync ─────────────────────────────────────────────────────────

    #[test]
    fn shutdown_sync_completes_without_panic() {
        let client = make_client();
        // Should not panic or block forever.
        client.shutdown_sync();
    }

    // ── async error paths ─────────────────────────────────────────────────────

    #[cfg(feature = "async")]
    #[test]
    fn send_async_invalid_payload_json_returns_config_error() {
        let client = make_client();
        let err = block_on(async {
            client
                .send("default".to_owned(), "not json {{{".to_owned())
                .await
        })
        .unwrap_err();
        assert!(
            matches!(err, WebhookFfiError::Config { .. }),
            "expected Config error, got {err:?}"
        );
    }

    #[cfg(feature = "async")]
    #[test]
    fn dispatch_async_invalid_payload_json_returns_config_error() {
        let client = make_client();
        let err = block_on(async { client.dispatch("not json".to_owned()).await }).unwrap_err();
        assert!(
            matches!(err, WebhookFfiError::Config { .. }),
            "expected Config error, got {err:?}"
        );
    }

    #[cfg(feature = "async")]
    #[test]
    fn shutdown_async_completes_without_panic() {
        let client = make_client();
        block_on(async { client.shutdown().await });
    }
}
