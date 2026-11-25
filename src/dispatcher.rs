use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::Arc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

use kanal::{AsyncReceiver, AsyncSender};
use serde::Serialize;
use serde_json::Value;

use crate::{
    delivery::deliver,
    error::WebhookError,
    hints::{WithHints, extract_hints},
    limiter::{Limiter, RetryPolicy},
    matcher::MatcherRegistry,
    platform::Platform,
    signal::Signal,
    stats::{Counters, DestinationStats},
    template::TemplateEngine,
};

type ErrorHandler = Arc<dyn Fn(WebhookError) + Send + Sync>;

enum DispatchJob {
    Send { template_name: String, data: Value },
    Flush { done: Arc<Signal> },
}

enum WorkerMsg {
    Payload(Value),
    Flush(Arc<Signal>),
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

impl fmt::Debug for Destination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Destination")
            .field("name", &self.name)
            .field("endpoint", &self.platform.redacted_endpoint())
            .finish()
    }
}

pub struct WebhookDispatcher {
    sender: AsyncSender<DispatchJob>,
    done: Arc<Signal>,
    matcher: MatcherRegistry,
    engine: TemplateEngine,
    stats: Arc<Vec<(String, Arc<Counters>)>>,
    /// Set when the dispatcher owns its runtime thread (`spawn_runtime`).
    thread: Option<JoinHandle<()>>,
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

    /// Register a new template at runtime.
    pub fn register_template(&self, name: &str, template: &str) -> Result<(), WebhookError> {
        self.engine.register(name, template)
    }

    /// Overwrite an existing template at runtime without restarting the dispatcher.
    pub fn update_template(&self, name: &str, template: &str) -> Result<(), WebhookError> {
        self.engine.update(name, template)
    }

    /// Remove a template at runtime.
    pub fn remove_template(&self, name: &str) {
        self.engine.remove(name);
    }

    /// Per-destination counters (sent, failed, dropped, retried, coalesced).
    pub fn stats(&self) -> HashMap<String, DestinationStats> {
        self.stats
            .iter()
            .map(|(name, c)| (name.clone(), c.snapshot()))
            .collect()
    }

    // ---- async API: waits for *enqueue* only (blocks while the queue is full) ----

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

    // ---- non-blocking API: never awaits, never blocks, needs no runtime ----

    /// Like [`send`](Self::send) but never blocks: if the queue is full the
    /// message is dropped (counted in every destination's `dropped`) and
    /// `Err(WebhookError::QueueFull)` is returned.
    pub fn try_send<T: Serialize + 'static>(&self, event: &T) -> Result<(), WebhookError> {
        let template_name = self.matcher.resolve(event).to_owned();
        self.try_push(template_name, event, None)
    }

    pub fn try_send_with_hints<T: Serialize + 'static>(
        &self,
        event: &T,
        hints: WithHints,
    ) -> Result<(), WebhookError> {
        let template_name = self.matcher.resolve(event).to_owned();
        self.try_push(template_name, event, Some(hints))
    }

    pub fn try_send_with_template<T: Serialize>(
        &self,
        template_name: &str,
        event: &T,
    ) -> Result<(), WebhookError> {
        self.try_push(template_name.to_owned(), event, None)
    }

    pub fn try_send_with_template_and_hints<T: Serialize>(
        &self,
        template_name: &str,
        event: &T,
        hints: WithHints,
    ) -> Result<(), WebhookError> {
        self.try_push(template_name.to_owned(), event, Some(hints))
    }

    // ---- flush / shutdown ----

    /// Waits for all currently queued jobs to finish processing before returning.
    /// Use this between phases where template mutations depend on prior sends completing.
    pub async fn flush(&self) -> Result<(), WebhookError> {
        let done = Signal::new();
        self.sender
            .send(DispatchJob::Flush { done: done.clone() })
            .await
            .map_err(|_| WebhookError::ChannelClosed)?;
        done.wait().await;
        Ok(())
    }

    /// Blocking [`flush`](Self::flush) for synchronous callers. Returns
    /// `Err(WebhookError::Timeout)` if the queue did not drain within `deadline`.
    pub fn flush_blocking(&self, deadline: Duration) -> Result<(), WebhookError> {
        let start = Instant::now();
        let done = Signal::new();
        self.sender
            .clone_sync()
            .send_timeout(DispatchJob::Flush { done: done.clone() }, deadline)
            .map_err(|e| match e {
                kanal::SendErrorTimeout::Timeout => WebhookError::Timeout,
                _ => WebhookError::ChannelClosed,
            })?;
        if done.wait_timeout(deadline.saturating_sub(start.elapsed())) {
            Ok(())
        } else {
            Err(WebhookError::Timeout)
        }
    }

    /// Closes the send side of the channel, drains all queued jobs, then resolves.
    /// Call this during graceful shutdown to ensure no messages are lost.
    pub async fn shutdown(self) {
        drop(self.sender);
        self.done.wait().await;
        if let Some(t) = self.thread {
            let _ = t.join();
        }
    }

    /// Blocking [`shutdown`](Self::shutdown). On `Err(WebhookError::Timeout)`
    /// the remaining queue keeps draining in the background.
    pub fn shutdown_blocking(self, deadline: Duration) -> Result<(), WebhookError> {
        drop(self.sender);
        if !self.done.wait_timeout(deadline) {
            return Err(WebhookError::Timeout);
        }
        if let Some(t) = self.thread {
            let _ = t.join();
        }
        Ok(())
    }

    fn prepare<T: Serialize>(
        &self,
        template_name: String,
        event: &T,
        hints: Option<WithHints>,
    ) -> Result<DispatchJob, WebhookError> {
        if !self.engine.has_template(&template_name) {
            return Err(WebhookError::TemplateNotFound(template_name));
        }

        let mut data = serde_json::to_value(event)?;

        if let Some(h) = hints {
            if let Some(obj) = data.as_object_mut() {
                obj.extend(h.map);
            }
        }

        Ok(DispatchJob::Send {
            template_name,
            data,
        })
    }

    async fn push<T: Serialize>(
        &self,
        template_name: String,
        event: &T,
        hints: Option<WithHints>,
    ) -> Result<(), WebhookError> {
        let job = self.prepare(template_name, event, hints)?;
        self.sender
            .send(job)
            .await
            .map_err(|_| WebhookError::ChannelClosed)
    }

    fn try_push<T: Serialize>(
        &self,
        template_name: String,
        event: &T,
        hints: Option<WithHints>,
    ) -> Result<(), WebhookError> {
        let job = self.prepare(template_name, event, hints)?;
        match self.sender.try_send(job) {
            Ok(true) => Ok(()),
            Ok(false) => {
                for (_, c) in self.stats.iter() {
                    Counters::add(&c.dropped, 1);
                }
                Err(WebhookError::QueueFull)
            }
            Err(_) => Err(WebhookError::ChannelClosed),
        }
    }
}

pub struct WebhookDispatcherBuilder {
    templates: Vec<(String, String)>,
    destinations: Vec<Destination>,
    default_template: &'static str,
    capacity: usize,
    timeout: Duration,
    retry: RetryPolicy,
    on_error: Option<ErrorHandler>,
}

impl WebhookDispatcherBuilder {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
            destinations: Vec::new(),
            default_template: "default",
            capacity: 1024,
            timeout: Duration::from_secs(5),
            retry: RetryPolicy::default(),
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

    /// Size of the intake queue and of each destination's queue.
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Per-request HTTP timeout (default 5 s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Retry policy for 429 / 5xx (default: 3 attempts, 500 ms backoff, 60 s max wait).
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    pub fn on_error(mut self, handler: impl Fn(WebhookError) + Send + Sync + 'static) -> Self {
        self.on_error = Some(Arc::new(handler));
        self
    }

    /// Builds the dispatcher on the caller's tokio runtime. Panics (like
    /// `tokio::spawn`) when called outside one; use [`spawn_runtime`](Self::spawn_runtime)
    /// from synchronous code.
    pub fn build(self) -> Result<WebhookDispatcher, WebhookError> {
        let (parts, run) = self.assemble()?;
        tokio::spawn(run);
        Ok(parts.finish(None))
    }

    /// Builds the dispatcher on its own thread running a current-thread tokio
    /// runtime, so synchronous callers need no runtime. Pair with
    /// [`try_send`](WebhookDispatcher::try_send),
    /// [`flush_blocking`](WebhookDispatcher::flush_blocking) and
    /// [`shutdown_blocking`](WebhookDispatcher::shutdown_blocking).
    pub fn spawn_runtime(self) -> Result<WebhookDispatcher, WebhookError> {
        let (parts, run) = self.assemble()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| WebhookError::Runtime(e.to_string()))?;
        let thread = std::thread::Builder::new()
            .name("chipa-webhooks".to_owned())
            .spawn(move || runtime.block_on(run))
            .map_err(|e| WebhookError::Runtime(e.to_string()))?;
        Ok(parts.finish(Some(thread)))
    }

    #[allow(clippy::type_complexity)]
    fn assemble(
        self,
    ) -> Result<
        (
            Parts,
            impl std::future::Future<Output = ()> + Send + 'static,
        ),
        WebhookError,
    > {
        let engine = TemplateEngine::new();
        for (name, template) in &self.templates {
            engine.register(name, template)?;
        }

        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(WebhookError::ClientBuild)?;

        let capacity = self.capacity.max(1);
        let (sender, receiver) = kanal::bounded_async(capacity);
        let done = Signal::new();

        let stats: Arc<Vec<(String, Arc<Counters>)>> = Arc::new(
            self.destinations
                .iter()
                .map(|d| (d.name.clone(), Arc::new(Counters::default())))
                .collect(),
        );

        let run = dispatch_loop(
            receiver,
            engine.clone(),
            self.destinations,
            stats.clone(),
            client,
            self.retry,
            capacity,
            self.on_error,
            done.clone(),
        );

        Ok((
            Parts {
                sender,
                done,
                matcher: MatcherRegistry::new(self.default_template),
                engine,
                stats,
            },
            run,
        ))
    }
}

struct Parts {
    sender: AsyncSender<DispatchJob>,
    done: Arc<Signal>,
    matcher: MatcherRegistry,
    engine: TemplateEngine,
    stats: Arc<Vec<(String, Arc<Counters>)>>,
}

impl Parts {
    fn finish(self, thread: Option<JoinHandle<()>>) -> WebhookDispatcher {
        WebhookDispatcher {
            sender: self.sender,
            done: self.done,
            matcher: self.matcher,
            engine: self.engine,
            stats: self.stats,
            thread,
        }
    }
}

impl Default for WebhookDispatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_loop(
    receiver: AsyncReceiver<DispatchJob>,
    engine: TemplateEngine,
    destinations: Vec<Destination>,
    stats: Arc<Vec<(String, Arc<Counters>)>>,
    client: reqwest::Client,
    policy: RetryPolicy,
    capacity: usize,
    on_error: Option<ErrorHandler>,
    done: Arc<Signal>,
) {
    // One worker per destination: a slow or rate-limited destination never
    // delays the others, and each owns its own limiter and backlog.
    let mut workers = Vec::with_capacity(destinations.len());
    let mut handles = Vec::with_capacity(destinations.len());
    for (dest, (_, counters)) in destinations.into_iter().zip(stats.iter()) {
        let (tx, rx) = kanal::bounded_async::<WorkerMsg>(capacity);
        handles.push(tokio::spawn(run_worker(
            rx,
            Worker {
                name: dest.name.clone(),
                limiter: Limiter::new(dest.platform.rate_limits()),
                platform: dest.platform.clone(),
                client: client.clone(),
                policy,
                counters: counters.clone(),
                on_error: on_error.clone(),
                capacity,
            },
        )));
        workers.push((dest, tx, counters.clone()));
    }

    while let Ok(job) = receiver.recv().await {
        match job {
            DispatchJob::Flush { done } => {
                let mut acks = Vec::new();
                for (_, tx, _) in &workers {
                    let ack = Signal::new();
                    if tx.send(WorkerMsg::Flush(ack.clone())).await.is_ok() {
                        acks.push(ack);
                    }
                }
                for ack in acks {
                    ack.wait().await;
                }
                done.fire();
            }
            DispatchJob::Send {
                template_name,
                mut data,
            } => {
                let hints = extract_hints(&mut data);

                let rendered = match engine.render(&template_name, &data) {
                    Ok(r) => r,
                    Err(e) => {
                        report_error(&on_error, e);
                        continue;
                    }
                };

                for (dest, tx, counters) in &workers {
                    let payload = dest.platform.build_payload(&rendered, &hints);
                    match tx.try_send(WorkerMsg::Payload(payload)) {
                        Ok(true) => {}
                        Ok(false) => {
                            Counters::add(&counters.dropped, 1);
                            report_error(
                                &on_error,
                                WebhookError::DestinationQueueFull(dest.name.clone()),
                            );
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }

    // Intake closed: let each worker drain its backlog, then signal completion.
    drop(workers);
    for h in handles {
        let _ = h.await;
    }
    done.fire();
}

struct Worker {
    name: String,
    platform: Arc<dyn Platform>,
    client: reqwest::Client,
    limiter: Limiter,
    policy: RetryPolicy,
    counters: Arc<Counters>,
    on_error: Option<ErrorHandler>,
    capacity: usize,
}

/// Moves queued messages into `pending` without waiting. Stops at a flush
/// marker (so nothing sent after a flush is delivered before it completes).
fn drain(
    rx: &AsyncReceiver<WorkerMsg>,
    pending: &mut VecDeque<Value>,
    flush: &mut Option<Arc<Signal>>,
    capacity: usize,
) {
    while flush.is_none() && pending.len() < capacity {
        match rx.try_recv() {
            Ok(Some(msg)) => absorb(msg, pending, flush),
            _ => break,
        }
    }
}

fn absorb(msg: WorkerMsg, pending: &mut VecDeque<Value>, flush: &mut Option<Arc<Signal>>) {
    match msg {
        WorkerMsg::Payload(p) => pending.push_back(p),
        WorkerMsg::Flush(ack) => *flush = Some(ack),
    }
}

async fn run_worker(rx: AsyncReceiver<WorkerMsg>, mut w: Worker) {
    let mut pending: VecDeque<Value> = VecDeque::new();
    let mut flush: Option<Arc<Signal>> = None;

    loop {
        if pending.is_empty() {
            if let Some(ack) = flush.take() {
                ack.fire();
            }
            match rx.recv().await {
                Ok(msg) => absorb(msg, &mut pending, &mut flush),
                Err(_) => break,
            }
        }
        drain(&rx, &mut pending, &mut flush, w.capacity);
        if pending.is_empty() {
            continue;
        }

        // If the rate limit makes us wait, more messages queue up meanwhile;
        // those can then be merged into a single message when the platform allows.
        let wait = w.limiter.wait_time(Instant::now());
        let waited = !wait.is_zero();
        if waited {
            tokio::time::sleep(wait).await;
            drain(&rx, &mut pending, &mut flush, w.capacity);
        }

        let (payload, count) = match waited
            .then(|| w.platform.coalesce(pending.make_contiguous()))
            .flatten()
        {
            Some(merged) if pending.len() > 1 => {
                let n = pending.len();
                pending.clear();
                Counters::add(&w.counters.coalesced, (n - 1) as u64);
                (merged, n)
            }
            _ => (pending.pop_front().expect("non-empty"), 1),
        };

        match deliver(
            &w.client,
            w.platform.as_ref(),
            &payload,
            &w.name,
            &mut w.limiter,
            &w.policy,
            &w.counters,
        )
        .await
        {
            Ok(()) => Counters::add(&w.counters.sent, count as u64),
            Err(e) => {
                Counters::add(&w.counters.failed, count as u64);
                report_error(&w.on_error, e);
            }
        }
    }
}

fn report_error(on_error: &Option<ErrorHandler>, e: WebhookError) {
    if let Some(handler) = on_error {
        handler(e);
    } else {
        tracing::warn!(error = %e, "webhook dispatch error");
    }
}
