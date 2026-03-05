use std::sync::Arc;

use futures::future::join_all;
use kanal::{AsyncReceiver, AsyncSender};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::oneshot;

use crate::{
    error::WebhookError,
    hints::{WithHints, extract_hints},
    matcher::MatcherRegistry,
    platform::{Platform, post},
    template::TemplateEngine,
};

struct DispatchJob {
    template_name: String,
    data: Value,
}

pub struct Destination {
    pub name: String,
    pub platform: Arc<dyn Platform>,
}

impl Destination {
    pub fn new(name: impl Into<String>, platform: impl Platform + 'static) -> Self {
        Self {
            name: name.into(),
            platform: Arc::new(platform),
        }
    }
}

pub struct WebhookDispatcher {
    sender: AsyncSender<DispatchJob>,
    done_rx: oneshot::Receiver<()>,
    matcher: MatcherRegistry,
}

impl WebhookDispatcher {
    pub fn builder() -> WebhookDispatcherBuilder {
        WebhookDispatcherBuilder::new()
    }

    /// Register a matcher rule for type `T`. When `send` is called with a `T`,
    /// this closure decides which template name to use.
    pub fn register_rule<T: 'static>(
        &mut self,
        rule: impl Fn(&T) -> &'static str + Send + Sync + 'static,
    ) {
        self.matcher.register(rule);
    }

    /// Send using the internal matcher to resolve the template name.
    pub async fn send<T: Serialize + 'static>(&self, event: &T) -> Result<(), WebhookError> {
        let template_name = self.matcher.resolve(event).to_owned();
        self.push(template_name, event, None).await
    }

    /// Send using the internal matcher, with platform hints (color, title, etc).
    pub async fn send_with_hints<T: Serialize + 'static>(
        &self,
        event: &T,
        hints: WithHints,
    ) -> Result<(), WebhookError> {
        let template_name = self.matcher.resolve(event).to_owned();
        self.push(template_name, event, Some(hints)).await
    }

    /// Send to an explicit template, bypassing the matcher entirely.
    pub async fn send_with_template<T: Serialize>(
        &self,
        template_name: &str,
        event: &T,
    ) -> Result<(), WebhookError> {
        self.push(template_name.to_owned(), event, None).await
    }

    /// Send to an explicit template with platform hints.
    pub async fn send_with_template_and_hints<T: Serialize>(
        &self,
        template_name: &str,
        event: &T,
        hints: WithHints,
    ) -> Result<(), WebhookError> {
        self.push(template_name.to_owned(), event, Some(hints))
            .await
    }

    /// Closes the send side of the channel, drains all queued jobs, then resolves.
    /// Call this during graceful shutdown to ensure no messages are lost.
    pub async fn shutdown(self) {
        drop(self.sender);
        let _ = self.done_rx.await;
    }

    async fn push<T: Serialize>(
        &self,
        template_name: String,
        event: &T,
        hints: Option<WithHints>,
    ) -> Result<(), WebhookError> {
        let mut data = serde_json::to_value(event)?;

        if let Some(h) = hints {
            if let Some(obj) = data.as_object_mut() {
                obj.extend(h.map);
            }
        }

        self.sender
            .send(DispatchJob {
                template_name,
                data,
            })
            .await
            .map_err(|_| WebhookError::ChannelClosed)
    }
}

pub struct WebhookDispatcherBuilder {
    templates: Vec<(String, String)>,
    destinations: Vec<Destination>,
    default_template: &'static str,
    capacity: usize,
    on_error: Option<Arc<dyn Fn(WebhookError) + Send + Sync>>,
}

impl WebhookDispatcherBuilder {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
            destinations: Vec::new(),
            default_template: "default",
            capacity: 1024,
            on_error: None,
        }
    }

    pub fn template(mut self, name: impl Into<String>, template: impl Into<String>) -> Self {
        self.templates.push((name.into(), template.into()));
        self
    }

    pub fn destination(mut self, dest: Destination) -> Self {
        self.destinations.push(dest);
        self
    }

    pub fn default_template(mut self, name: &'static str) -> Self {
        self.default_template = name;
        self
    }

    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn on_error(mut self, handler: impl Fn(WebhookError) + Send + Sync + 'static) -> Self {
        self.on_error = Some(Arc::new(handler));
        self
    }

    pub fn build(self) -> Result<WebhookDispatcher, handlebars::TemplateError> {
        let mut engine = TemplateEngine::new();
        for (name, template) in &self.templates {
            engine.register(name, template)?;
        }

        let matcher = MatcherRegistry::new(self.default_template);

        let (sender, receiver) = kanal::bounded_async(self.capacity);
        let (done_tx, done_rx) = oneshot::channel();

        let destinations: Arc<Vec<Destination>> = Arc::new(self.destinations);
        let on_error = self.on_error;

        tokio::spawn(dispatch_loop(
            receiver,
            Arc::new(engine),
            destinations,
            on_error,
            done_tx,
        ));

        Ok(WebhookDispatcher {
            sender,
            done_rx,
            matcher,
        })
    }
}

impl Default for WebhookDispatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

async fn dispatch_loop(
    receiver: AsyncReceiver<DispatchJob>,
    engine: Arc<TemplateEngine>,
    destinations: Arc<Vec<Destination>>,
    on_error: Option<Arc<dyn Fn(WebhookError) + Send + Sync>>,
    done_tx: oneshot::Sender<()>,
) {
    let client = reqwest::Client::new();

    while let Ok(job) = receiver.recv().await {
        let mut data = job.data;
        let hints = extract_hints(&mut data);

        let rendered = match engine.render(&job.template_name, &data) {
            Ok(r) => r,
            Err(e) => {
                report_error(&on_error, e);
                continue;
            }
        };

        let rendered = Arc::new(rendered);
        let hints = Arc::new(hints);

        let futs = destinations.iter().map(|dest| {
            let client = client.clone();
            let rendered = Arc::clone(&rendered);
            let hints = Arc::clone(&hints);
            let dest_name = dest.name.clone();
            let on_error = on_error.clone();
            let platform = Arc::clone(&dest.platform);

            async move {
                if let Err(e) =
                    post(&client, platform.as_ref(), &rendered, &hints, &dest_name).await
                {
                    report_error(&on_error, e);
                }
            }
        });

        join_all(futs).await;
    }

    // Channel drained and closed — signal graceful shutdown complete.
    let _ = done_tx.send(());
}

fn report_error(on_error: &Option<Arc<dyn Fn(WebhookError) + Send + Sync>>, e: WebhookError) {
    if let Some(handler) = on_error {
        handler(e);
    } else {
        tracing::warn!(error = %e, "webhook dispatch error");
    }
}
