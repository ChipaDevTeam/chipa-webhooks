//! Behaviour tests against a throwaway local HTTP server (no network needed).

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chipa_webhooks::{
    Destination, Discord, Generic, RateLimit, RetryPolicy, WebhookDispatcher, WebhookError,
};
use serde::Serialize;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[derive(Clone, Debug)]
struct Seen {
    at: Instant,
    body: Value,
}

struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
}

impl Reply {
    fn ok() -> Self {
        Self {
            status: 204,
            headers: vec![],
            body: String::new(),
        }
    }
    fn status(status: u16) -> Self {
        Self {
            status,
            headers: vec![],
            body: String::new(),
        }
    }
}

struct Server {
    port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
    /// `script(n)` decides the reply for the n-th request (0-based).
    async fn start(script: impl Fn(usize) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let script = Arc::new(script);
        let seen2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let seen = seen2.clone();
                let script = script.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let (head_end, len) = loop {
                        let n = sock.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&buf[..i]).to_lowercase();
                            let len = head
                                .lines()
                                .find_map(|l| l.strip_prefix("content-length:"))
                                .and_then(|v| v.trim().parse::<usize>().ok())
                                .unwrap_or(0);
                            break (i + 4, len);
                        }
                    };
                    while buf.len() < head_end + len {
                        let n = sock.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    let body = serde_json::from_slice(&buf[head_end..]).unwrap_or(Value::Null);
                    let n = {
                        let mut s = seen.lock().unwrap();
                        s.push(Seen {
                            at: Instant::now(),
                            body,
                        });
                        s.len() - 1
                    };
                    let reply = script(n);
                    let mut out = format!(
                        "HTTP/1.1 {} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                        reply.status,
                        reply.body.len()
                    );
                    for (k, v) in reply.headers {
                        out.push_str(&format!("{k}: {v}\r\n"));
                    }
                    out.push_str("\r\n");
                    out.push_str(&reply.body);
                    let _ = sock.write_all(out.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        Self { port, seen }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/hook/SECRETTOKEN", self.port)
    }

    fn discord_url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/api/webhooks/123/SECRETTOKEN",
            self.port
        )
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

#[derive(Serialize)]
struct Msg {
    text: String,
}

fn msg(i: usize) -> Msg {
    Msg {
        text: format!("m{i}"),
    }
}

fn fast_retry() -> RetryPolicy {
    RetryPolicy {
        max_attempts: 3,
        base_backoff: Duration::from_millis(50),
        max_wait: Duration::from_secs(60),
    }
}

fn errors() -> (Arc<Mutex<Vec<String>>>, impl Fn(WebhookError) + Send + Sync) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    (log, move |e| {
        log2.lock().unwrap().push(format!("{e} / {e:?}"))
    })
}

#[tokio::test]
async fn a_429_waits_retry_after() {
    let server = Server::start(|n| {
        if n == 0 {
            Reply {
                status: 429,
                headers: vec![("retry-after", "0.4".into())],
                body: r#"{"retry_after": 99}"#.into(),
            }
        } else {
            Reply::ok()
        }
    })
    .await;

    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new("g", Generic::new(server.url())))
        .retry(fast_retry())
        .build()
        .unwrap();
    d.send(&msg(1)).await.unwrap();
    d.flush().await.unwrap();

    let seen = server.seen();
    assert_eq!(seen.len(), 2);
    let gap = seen[1].at - seen[0].at;
    assert!(gap >= Duration::from_millis(380), "gap {gap:?}");
    assert!(
        gap < Duration::from_secs(5),
        "header must win over body: {gap:?}"
    );
    let s = d.stats()["g"];
    assert_eq!((s.sent, s.failed, s.retried), (1, 0, 1));
}

#[tokio::test]
async fn retry_after_comes_from_body_when_no_header_and_is_clamped() {
    let server = Server::start(|n| {
        if n == 0 {
            Reply {
                status: 429,
                headers: vec![],
                body: r#"{"retry_after": 3600}"#.into(),
            }
        } else {
            Reply::ok()
        }
    })
    .await;
    let policy = RetryPolicy {
        max_wait: Duration::from_millis(300),
        ..fast_retry()
    };
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new("g", Generic::new(server.url())))
        .retry(policy)
        .build()
        .unwrap();
    let t = Instant::now();
    d.send(&msg(1)).await.unwrap();
    d.flush().await.unwrap();
    assert!(t.elapsed() < Duration::from_secs(3));
    assert_eq!(server.seen().len(), 2);
}

#[tokio::test]
async fn server_errors_are_retried_then_reported_without_the_token() {
    let server = Server::start(|_| Reply::status(503)).await;
    let (log, on_error) = errors();
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new("g", Generic::new(server.url())))
        .retry(fast_retry())
        .on_error(on_error)
        .build()
        .unwrap();
    d.send(&msg(1)).await.unwrap();
    d.flush().await.unwrap();

    assert_eq!(server.seen().len(), 3);
    let s = d.stats()["g"];
    assert_eq!((s.sent, s.failed, s.retried), (0, 1, 2));
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert!(log[0].contains("503"), "{}", log[0]);
    assert!(!log[0].contains("SECRETTOKEN"), "{}", log[0]);
}

#[tokio::test]
async fn client_errors_are_reported_not_retried() {
    let server = Server::start(|_| Reply::status(400)).await;
    let (log, on_error) = errors();
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new("g", Generic::new(server.url())))
        .retry(fast_retry())
        .on_error(on_error)
        .build()
        .unwrap();
    d.send(&msg(1)).await.unwrap();
    d.flush().await.unwrap();
    assert_eq!(server.seen().len(), 1);
    assert_eq!(log.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn webhook_url_is_redacted_on_transport_errors() {
    // Bind then drop, so the port refuses connections.
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let url = format!("http://127.0.0.1:{port}/api/webhooks/123/SECRETTOKEN");
    let (log, on_error) = errors();
    let discord = Discord::new(url);
    assert!(!format!("{discord:?}").contains("SECRETTOKEN"));
    let dest = Destination::new("discord", discord);
    assert!(!format!("{dest:?}").contains("SECRETTOKEN"));

    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(dest)
        .on_error(on_error)
        .build()
        .unwrap();
    d.send(&msg(1)).await.unwrap();
    d.flush().await.unwrap();
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert!(!log[0].contains("SECRETTOKEN"), "{}", log[0]);
    assert_eq!(d.stats()["discord"].failed, 1);
}

#[tokio::test]
async fn request_timeout_is_enforced() {
    // Accepts connections but never answers.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((s, _)) = listener.accept().await {
            held.push(s);
        }
    });
    let (log, on_error) = errors();
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new(
            "g",
            Generic::new(format!("http://127.0.0.1:{port}/x")),
        ))
        .timeout(Duration::from_millis(200))
        .on_error(on_error)
        .build()
        .unwrap();
    let t = Instant::now();
    d.send(&msg(1)).await.unwrap();
    d.flush().await.unwrap();
    assert!(t.elapsed() < Duration::from_secs(3));
    assert_eq!(log.lock().unwrap().len(), 1);
}

#[test]
fn burst_is_coalesced_within_the_limit() {
    // Sync test on purpose: the dispatcher owns its runtime thread.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(Server::start(|_| Reply::ok()));

    let discord = Discord::new(server.discord_url())
        .with_rate_limits(vec![RateLimit::new(2, Duration::from_millis(600))]);
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new("discord", discord))
        .spawn_runtime()
        .unwrap();

    const N: usize = 16;
    for i in 0..N {
        d.try_send(&msg(i)).unwrap();
    }
    d.flush_blocking(Duration::from_secs(10)).unwrap();

    let seen = server.seen();
    assert!(
        seen.len() < N,
        "expected coalescing, got {} requests",
        seen.len()
    );
    let mut accounted = 0;
    for s in &seen {
        let embeds = s.body["embeds"].as_array().unwrap().len();
        assert!(embeds <= 10);
        assert_eq!(s.body["allowed_mentions"]["parse"], serde_json::json!([]));
        accounted += embeds;
        if let Some(c) = s.body["content"].as_str() {
            accounted += c
                .strip_prefix('+')
                .and_then(|r| r.strip_suffix(" more"))
                .and_then(|n| n.parse::<usize>().ok())
                .expect("content is '+k more'");
        }
    }
    assert_eq!(accounted, N);
    let st = d.stats()["discord"];
    assert_eq!((st.sent, st.failed, st.dropped), (N as u64, 0, 0));
    assert!(st.coalesced > 0);
    d.shutdown_blocking(Duration::from_secs(5)).unwrap();
}

#[test]
fn sync_host_delivers_without_a_caller_runtime() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(Server::start(|_| Reply::ok()));
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new("g", Generic::new(server.url())))
        .spawn_runtime()
        .unwrap();
    // Generic has no limits: everything goes out immediately.
    for i in 0..5 {
        d.try_send(&msg(i)).unwrap();
    }
    d.flush_blocking(Duration::from_secs(5)).unwrap();
    assert_eq!(server.seen().len(), 5);
    d.shutdown_blocking(Duration::from_secs(5)).unwrap();
}

#[test]
fn try_send_never_blocks_and_counts_drops() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // Slow server so queues back up.
    let server = rt.block_on(Server::start(|_| Reply::ok()));
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new(
            "discord",
            Discord::new(server.discord_url())
                .with_rate_limits(vec![RateLimit::new(1, Duration::from_secs(30))]),
        ))
        .capacity(4)
        .spawn_runtime()
        .unwrap();

    let t = Instant::now();
    let mut full = 0u64;
    for i in 0..20_000 {
        match d.try_send(&msg(i)) {
            Ok(()) => {}
            Err(WebhookError::QueueFull) => full += 1,
            Err(e) => panic!("unexpected {e}"),
        }
    }
    assert!(t.elapsed() < Duration::from_secs(5), "try_send blocked");
    assert!(full > 0, "queue of 4 must overflow under 20k sends");
    assert!(d.stats()["discord"].dropped >= full);
    // Don't wait out the 30 s rate limit.
    assert!(matches!(
        d.shutdown_blocking(Duration::from_millis(200)),
        Err(WebhookError::Timeout)
    ));
}

#[tokio::test]
async fn async_flush_and_shutdown_drain_everything() {
    let server = Server::start(|_| Reply::ok()).await;
    let d = WebhookDispatcher::builder()
        .template("default", "{{text}}")
        .destination(Destination::new("a", Generic::new(server.url())))
        .destination(Destination::new("b", Generic::new(server.url())))
        .build()
        .unwrap();
    for i in 0..10 {
        d.send(&msg(i)).await.unwrap();
    }
    d.flush().await.unwrap();
    assert_eq!(server.seen().len(), 20);
    d.send(&msg(99)).await.unwrap();
    d.shutdown().await;
    assert_eq!(server.seen().len(), 22);
}
