# webhooks-uniffi — Implementation Spec

## Purpose

Thin FFI binding layer over `chipa-webhooks` exposing a `WebhookClient` object to Python,
Kotlin, Swift, Java, C++, and C via UniFFI (and manual `extern "C"` for C).

---

## Constraints

| Constraint | Value | Reason |
|---|---|---|
| `uniffi` pin | `0.29.4` | `uniffi-bindgen-cpp` v0.8.1 and `uniffi-bindgen-java` 0.2.1 both target 0.29.x |
| C++ async | ❌ never | `uniffi-bindgen-cpp` explicitly does not support async functions |
| C | ❌ no UniFFI | No backend exists; use manual `extern "C"` + `cbindgen` |
| Rust edition | 2024 | Match workspace |
| Error philosophy | Helpful > terse | Every error must tell the user what went wrong, where, and what to try next |

---

## Crate layout

```
webhooks-uniffi/
  Cargo.toml
  uniffi.toml
  cbindgen.toml
  src/
    lib.rs          ← uniffi::setup_scaffolding!(), re-exports, feature gates
    client.rs       ← WebhookClient (the single exported object)
    config.rs       ← JSON config parsing (DestinationConfig, CredentialsConfig)
    error.rs        ← WebhookFfiError (UniFFI error enum, rootcause formatting)
    runtime.rs      ← Lazy tokio runtime for blocking _sync variants
    ffi_c.rs        ← extern "C" opaque-handle API (C and C++ fallback)
  uniffi-bindgen.rs ← thin binary: calls uniffi::uniffi_bindgen_main()
```

---

## Coding rules

- **1 function = 1 task.** No function does parsing AND dispatching AND error mapping.
- **No repetition.** Every conversion, every error format string, every config path lives in
  exactly one place. If you need it twice, extract it.
- **OOP discipline.** `WebhookClient` is the only public object. Everything else is either a
  pure data type (record/enum) or a private helper. No God structs.
- **Minimal surface.** Do not expose anything the caller does not need. Internal types stay
  internal. UniFFI exports are the minimum viable API.
- **No `unwrap` in library code.** Every `?` must reach a typed error variant. Panics are
  forbidden except inside the C layer's null-pointer guard (document it).

---

## Feature flags

```toml
[features]
default = []

# Enables async exported functions (Python, Kotlin, Swift, Java).
# When off only *_sync variants are compiled and exported (C++, C).
async = ["uniffi/tokio"]

# Documentation-only flags — no code is conditionally compiled.
# Enable all in [package.metadata.docs.rs] so docs.rs shows everything.
python  = []
kotlin  = []
swift   = []
java    = []
cpp     = []
c       = []
```

`async` is the **only** flag that changes compiled output. Language flags exist solely so
`cfg_attr(feature = "X", doc = "...")` can show language-specific usage examples in rustdoc.

---

## Public API (what UniFFI sees)

### `WebhookClient`  ← `uniffi::Object`

```
Arc<WebhookClient>
  ::new(config_json: String) -> Result<Arc<Self>, WebhookFfiError>

  // explicit template routing
  ::send(template_name: String, payload_json: String) -> Result<(), WebhookFfiError>        [async]
  ::send_sync(template_name: String, payload_json: String) -> Result<(), WebhookFfiError>

  // matcher-based routing (walks registered matchers, falls back to "default")
  ::dispatch(payload_json: String) -> Result<(), WebhookFfiError>                           [async]
  ::dispatch_sync(payload_json: String) -> Result<(), WebhookFfiError>

  // matcher registry
  ::register_matcher(matcher: Box<dyn MatcherCallback>)

  ::shutdown()                                                                               [async]
  ::shutdown_sync()
```

`new` is a UniFFI constructor (returns `Arc<Self>`).
`send` / `dispatch` / `shutdown` are exported async when feature `async` is on; otherwise
only the `_sync` variants are exported.
`register_matcher` is always synchronous — registering a callback never needs to be async.

### `WebhookFfiError`  ← `uniffi::Error` enum

```
enum WebhookFfiError {
    Config    { message: String }   // bad JSON, unknown platform, missing field
    Template  { message: String }   // register/render failure
    Dispatch  { message: String }   // HTTP, channel, serialization
    Internal  { message: String }   // anything that should never happen
}
```

Each variant carries a single `message: String` built by the error formatting layer
(see Error handling below). UniFFI maps each variant to a typed exception in every language.

### `MatcherCallback`  ← `uniffi::callback_interface`

```rust
#[uniffi::export(callback_interface)]
pub trait MatcherCallback: Send + Sync {
    fn match_template(&self, payload_json: String) -> Option<String>;
}
```

The foreign language implements this trait as a class. `match_template` receives the raw
JSON payload string and returns either a template name or `None`/`null`/`nil` to pass to the
next registered matcher. The first non-None result wins. If all matchers return None the
dispatcher falls back to the `"default"` template.

UniFFI callback interfaces are **always synchronous** — `match_template` must not block or
perform I/O. It should only inspect the JSON string and return a name. Parsing the JSON is
the caller's responsibility inside their implementation.

---

## Config JSON contract

Callers pass a single JSON object to `WebhookClient::new`:

```json
{
  "destinations": [
    {
      "name": "my-discord",
      "credentials": { "type": "discord", "webhook_url": "https://..." }
    },
    {
      "name": "my-telegram",
      "credentials": { "type": "telegram", "token": "...", "chat_id": -1001234 }
    }
  ],
  "templates": {
    "default": "{{event}}: {{message}}"
  },
  "capacity": 1024
}
```

`capacity` is optional (defaults to 1024).
`templates` is optional (caller can register templates after construction via future extension).
At least one destination is required.

### `CredentialsConfig`  ← `serde` adjacently tagged

```rust
#[serde(tag = "type", rename_all = "snake_case")]
enum CredentialsConfig {
    Discord  { webhook_url: String },
    Slack    { webhook_url: String },
    Telegram { token: String, chat_id: i64 },
    Ntfy     { url: String, topic: String },
    Generic  { url: String },
}
```

`tag = "type"` means the JSON field `"type"` selects the variant.
`rename_all = "snake_case"` means JSON uses `"discord"`, `"telegram"` etc.

Each variant maps 1-to-1 to a platform struct in `chipa-webhooks`. The conversion function
`CredentialsConfig → Arc<dyn Platform>` lives in `config.rs` and nowhere else.

### `DestinationConfig`

```rust
struct DestinationConfig {
    name: String,
    credentials: CredentialsConfig,
}
```

`into_destination()` on this struct produces a `chipa_webhooks::Destination`. One method,
one task.

### `ClientConfig`

```rust
struct ClientConfig {
    destinations: Vec<DestinationConfig>,
    templates: Option<HashMap<String, String>>,
    capacity: Option<usize>,
}
```

`ClientConfig::parse(json: &str) -> Result<Self, WebhookFfiError>` is the single entry point
for all config parsing. It validates:

1. Valid JSON (serde_json parse error → `Config` variant with the serde message)
2. At least one destination present (explicit check with explicit message)
3. No duplicate destination names (explicit check with the conflicting name in the message)

---

## Error handling

### Philosophy

Errors must be **relaxing to fix**. That means:

- The message tells you **what** failed ("template 'alerts' not found")
- The message tells you **why** ("no template was registered under that name")
- The message tells you **what to do** ("register it with the `templates` key in your config JSON")
- The full rootcause chain is always included

### Mechanism

`WebhookError` (from `chipa-webhooks`) → `rootcause::Report` → formatted string → `WebhookFfiError` variant.

```rust
// error.rs
fn format_report(err: impl std::error::Error) -> String {
    use rootcause::prelude::*;
    let report: Report = err.into();
    format!("{report}")
}

fn into_config_error(err: impl std::error::Error, context: &str) -> WebhookFfiError {
    WebhookFfiError::Config {
        message: format!("{context}\n\nCaused by:\n{}", format_report(err)),
    }
}

fn into_dispatch_error(err: WebhookError) -> WebhookFfiError { ... }
fn into_template_error(err: WebhookError) -> WebhookFfiError { ... }
```

One conversion function per variant. Each adds a human context string before the chain.

### Example message a Python user would see

```
WebhookFfiError.Dispatch: HTTP request to destination "my-discord" failed

Caused by:
 ● http error sending to my-discord: ...
 ├ src/client.rs:42:18
 │
 ● connection refused (os error 111)
 ╰ ...
```

### Matcher storage in `client.rs`

```rust
struct WebhookClient {
    dispatcher: WebhookDispatcher,
    matchers: RwLock<Vec<Box<dyn MatcherCallback>>>,
}
```

`register_matcher` acquires a write lock and pushes. `dispatch`/`dispatch_sync` acquire a
read lock, iterate, and call `match_template` on each until one returns `Some`. The resolved
template name is then forwarded to the underlying `dispatcher.send_with_template`. This is
the entire routing logic — no new files, no new types beyond the callback interface.

The `RwLock` is `std::sync::RwLock` (not tokio's) because `match_template` is synchronous
and the lock is held only for the duration of the linear scan, which is always short.

### `on_error` inside the dispatcher

`WebhookDispatcher` accepts an `on_error` callback for background dispatch errors (errors
that happen after `send` returns, inside the async loop). Since foreign languages can't
pass closures over UniFFI, `WebhookClient` installs its own internal `on_error` handler
that logs via `tracing::error!` with the full rootcause-formatted string. This is not
configurable from the foreign side — it is always on.

---

## Async / blocking split

```
                   ┌──────────────────────────┐
                   │       client.rs          │
                   │                          │
  async feature ──►│  send() / dispatch() / shutdown()      │ ← #[uniffi::export] async fn
                   │                                         │
  always present  ►│  send_sync() / dispatch_sync()         │ ← runtime.rs::block_on(...)
                   │  shutdown_sync() / register_matcher()   │
                   └──────────────────────────┘
```

`runtime.rs` exposes a single function:

```rust
pub(crate) fn block_on<F: Future>(f: F) -> F::Output {
    RUNTIME.block_on(f)
}
```

Where `RUNTIME` is a `std::sync::OnceLock<tokio::runtime::Runtime>` initialised once with
`Runtime::new()`. One place, zero repetition.

---

## C layer (`ffi_c.rs`)

UniFFI does not generate C bindings. This module provides a minimal opaque-handle API:

```c
// Opaque handle — never dereference from C.
typedef struct WebhookClientHandle WebhookClientHandle;

// Returns NULL on error; writes a null-terminated UTF-8 error string into `err_buf`
// (up to `err_buf_len` bytes including the null terminator).
WebhookClientHandle* webhook_client_new(const char* config_json,
                                        char* err_buf, size_t err_buf_len);

// Returns 0 on success; nonzero on error (writes to err_buf).
int webhook_client_send(WebhookClientHandle* handle,
                        const char* template_name,
                        const char* payload_json,
                        char* err_buf, size_t err_buf_len);

// Matcher-based routing. Walks registered C matchers in order, falls back to "default".
// Returns 0 on success; nonzero on error (writes to err_buf).
int webhook_client_dispatch(WebhookClientHandle* handle,
                             const char* payload_json,
                             char* err_buf, size_t err_buf_len);

// Registers a matcher function. Called once per rule at startup.
// `matcher_fn` receives the payload JSON and returns a template name, or NULL to pass
// through. The pointer must remain valid for the lifetime of the handle.
typedef const char* (*webhook_matcher_fn)(const char* payload_json);
void webhook_client_register_matcher(WebhookClientHandle* handle,
                                     webhook_matcher_fn matcher_fn);

// Always succeeds. Frees the handle. Do not use handle after this call.
void webhook_client_free(WebhookClientHandle* handle);
```

Rules:
- Null-pointer check on `handle` at entry of every function. If null: write a message to
  `err_buf` and return early. **This is the only permitted panic-adjacent guard.**
- All blocking. Uses `runtime::block_on` internally — same runtime as `_sync` methods.
- `cbindgen` generates `webhooks.h` from this module.
- No heap allocation visible to the caller. The client is `Box::into_raw` / `Box::from_raw`.

---

## Documentation strategy

Every public item gets:

1. A language-agnostic summary (the first `///` line — always present).
2. Language-specific usage blocks under `cfg_attr` doc gates.

Pattern:

```rust
/// Send a webhook using the named template and a JSON payload.
///
/// `payload_json` must be a valid JSON object string.
#[cfg_attr(feature = "python",  doc = "### Python\n```python\nawait client.send('alerts', json.dumps(event))\n```")]
#[cfg_attr(feature = "kotlin",  doc = "### Kotlin\n```kotlin\nclient.send(\"alerts\", gson.toJson(event))\n```")]
#[cfg_attr(feature = "swift",   doc = "### Swift\n```swift\ntry await client.send(templateName: \"alerts\", payloadJson: encoded)\n```")]
#[cfg_attr(feature = "java",    doc = "### Java\n```java\nclient.send(\"alerts\", payload).get();\n```")]
#[cfg_attr(feature = "cpp",     doc = "### C++\n```cpp\nclient->send_sync(\"alerts\", payload);\n```")]
#[cfg_attr(feature = "c",       doc = "### C\n```c\nwebhook_client_send(handle, \"alerts\", payload, err, sizeof(err));\n```")]
```

`[package.metadata.docs.rs]` in `Cargo.toml` enables all language features so docs.rs
renders the full block:

```toml
[package.metadata.docs.rs]
features = ["async", "python", "kotlin", "swift", "java", "cpp", "c"]
```

---

## `uniffi-bindgen.rs` (binary entrypoint)

```rust
fn main() {
    uniffi::uniffi_bindgen_main()
}
```

That is the entire file. Run via:

```sh
cargo run --features uniffi/cli --bin uniffi-bindgen -- generate \
  --library target/release/libwebhooks_uniffi.so \
  --language python \
  --out-dir bindings/python
```

---

## `Cargo.toml` structure

```toml
[package]
name    = "webhooks-uniffi"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "staticlib"]

[[bin]]
name = "uniffi-bindgen"
path = "uniffi-bindgen.rs"

[dependencies]
chipa-webhooks    = { path = ".." }
uniffi            = { version = "0.29.4", features = ["cargo-metadata"] }
tokio             = { version = "1", features = ["rt-multi-thread"] }
serde             = { version = "1", features = ["derive"] }
serde_json        = "1"
thiserror         = "2"
rootcause         = "0.12.1"

[build-dependencies]
cbindgen = "0.27"           # generates webhooks.h from ffi_c.rs

[package.metadata.docs.rs]
features = ["async", "python", "kotlin", "swift", "java", "cpp", "c"]
```

---

## `uniffi.toml`

```toml
[bindings.python]
cdylib_name = "webhooks_uniffi"

[bindings.kotlin]
package_name = "io.chipawebhooks"

[bindings.swift]
module_name = "ChipaWebhooks"

[bindings.java]
package_name = "io.chipawebhooks"
```

---

## Binding generation commands (one per language)

```sh
# Build the library first (always required)
cargo build --release

LIB=target/release/libwebhooks_uniffi.so   # .dylib on macOS, .dll on Windows

# Python
cargo run --features uniffi/cli --bin uniffi-bindgen -- generate \
  --library $LIB --language python --out-dir bindings/python

# Kotlin
cargo run --features uniffi/cli --bin uniffi-bindgen -- generate \
  --library $LIB --language kotlin --out-dir bindings/kotlin

# Swift
cargo run --features uniffi/cli --bin uniffi-bindgen -- generate \
  --library $LIB --language swift --out-dir bindings/swift

# Java  (requires uniffi-bindgen-java installed)
cargo install uniffi-bindgen-java \
  --git https://github.com/IronCoreLabs/uniffi-bindgen-java
uniffi-bindgen-java generate --library $LIB --out-dir bindings/java

# C++  (requires uniffi-bindgen-cpp installed — must target uniffi 0.29.4)
cargo install uniffi-bindgen-cpp \
  --git https://github.com/NordSecurity/uniffi-bindgen-cpp \
  --tag v0.8.1+v0.29.4
uniffi-bindgen-cpp --library $LIB --out-dir bindings/cpp

# C  (header generated by build.rs via cbindgen)
# webhooks.h is written to bindings/c/ during cargo build
```

---

## Build script (`build.rs`)

One task: generate `webhooks.h` for C consumers via `cbindgen`.

```rust
fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(cbindgen::Config::from_file("cbindgen.toml").unwrap())
        .generate()
        .expect("cbindgen failed")
        .write_to_file("bindings/c/webhooks.h");
}
```

---

## Implementation order

Each step is independently verifiable before the next begins.

```
1. Cargo.toml + lib.rs skeleton + uniffi::setup_scaffolding!()
   → verify: cargo check passes

2. error.rs: WebhookFfiError enum + format_report + conversion fns
   → verify: all error paths compile, format strings look right in a unit test

3. config.rs: CredentialsConfig, DestinationConfig, ClientConfig::parse
   → verify: unit tests for valid JSON, missing destinations, duplicate names, bad type

4. runtime.rs: RUNTIME OnceLock + block_on wrapper
   → verify: a sync call to a trivial async fn works in a unit test

5. client.rs: WebhookClient::new + send_sync + shutdown_sync
   → verify: integration test against a mock HTTP server (no UniFFI yet)

6. client.rs: add async send + shutdown behind feature = "async"
   → verify: cargo check --features async passes; cargo check (no features) still passes

7. uniffi proc-macro annotations on client.rs + WebhookFfiError
   → verify: cargo run --bin uniffi-bindgen -- generate --library ... --language python succeeds

8. Smoke-test each language binding (python -c, kotlinc, swift build, javac)

9. client.rs: MatcherCallback callback interface + matchers RwLock field
              + register_matcher + dispatch/dispatch_sync
   → verify: unit test registers two matchers, dispatch resolves correct template;
             dispatch with no matchers falls back to "default"

10. ffi_c.rs + build.rs + cbindgen.toml
    → verify: webhooks.h is generated; compile a trivial C program that registers a
              matcher_fn and calls webhook_client_dispatch

11. Doc comments with cfg_attr language blocks on every public item
    → verify: cargo doc --features "async python kotlin swift java cpp c" renders correctly
```

---

## What this spec does NOT cover

- Gradle / SPM / pip packaging (deployment, not implementation)
- Cross-compilation toolchain setup (target-specific, document separately)
- WASM (`wasm-unstable-single-threaded` feature of UniFFI — out of scope)