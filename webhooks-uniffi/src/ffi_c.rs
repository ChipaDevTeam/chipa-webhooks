//! Manual `extern "C"` API for C and C++ consumers.
//!
//! UniFFI does not generate C bindings. This module provides a minimal opaque-handle
//! API that mirrors the sync surface of [`WebhookClient`]. `cbindgen` generates
//! `bindings/c/webhooks.h` from this module during the build step.
//!
//! # Safety contract
//!
//! - `handle` must be a pointer returned by [`webhook_client_new`] that has not yet
//!   been passed to [`webhook_client_free`].
//! - `config_json`, `template_name`, `payload_json` must be valid null-terminated
//!   UTF-8 strings for the duration of the call.
//! - `err_buf` must point to a writable buffer of at least `err_buf_len` bytes.
//!   Passing `NULL` for `err_buf` (with `err_buf_len = 0`) is safe — errors are
//!   silently dropped in that case.
//! - `matcher_fn` passed to [`webhook_client_register_matcher`] must remain valid
//!   for the lifetime of the handle.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use crate::client::WebhookClient;

// ── opaque handle ─────────────────────────────────────────────────────────────

/// Opaque handle to a `WebhookClient`. Never dereference from C.
pub struct WebhookClientHandle {
    client: std::sync::Arc<WebhookClient>,
    /// C-side matcher functions registered via [`webhook_client_register_matcher`].
    matchers: std::sync::Mutex<Vec<WebhookMatcherFn>>,
}

/// C matcher function pointer type.
///
/// Receives the payload JSON as a null-terminated UTF-8 string.
/// Return a null-terminated template name string to select that template,
/// or `NULL` to pass through to the next registered matcher.
///
/// The returned pointer must remain valid until the next call to any
/// `webhook_client_*` function on this handle (i.e. it may be a static string
/// literal or a thread-local buffer, but not a stack allocation that goes out
/// of scope before the caller returns).
pub type WebhookMatcherFn = unsafe extern "C" fn(payload_json: *const c_char) -> *const c_char;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Write `msg` into `buf` (up to `len` bytes including null terminator).
/// Does nothing if `buf` is null or `len` is 0.
fn write_err(buf: *mut c_char, len: usize, msg: &str) {
    if buf.is_null() || len == 0 {
        return;
    }
    let bytes = msg.as_bytes();
    let copy_len = bytes.len().min(len - 1);
    // SAFETY: caller contract guarantees buf points to a writable region of `len` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, copy_len);
        *buf.add(copy_len) = 0;
    }
}

/// Convert a raw `*const c_char` to a `&str`, returning an error string on failure.
///
/// # Safety
/// `ptr`, if non-null, must point to a valid null-terminated string for `'a`.
unsafe fn cstr_to_str<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err(format!("{name} must not be NULL"));
    }
    unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .map_err(|_| format!("{name} is not valid UTF-8"))
    }
}

// ── public API ────────────────────────────────────────────────────────────────

/// Create a new `WebhookClient` from a JSON config string.
///
/// Returns `NULL` on error and writes a null-terminated UTF-8 error message
/// into `err_buf` (up to `err_buf_len` bytes including the null terminator).
/// On success returns an opaque handle that must eventually be passed to
/// [`webhook_client_free`].
///
/// # Safety
/// See module-level safety contract.
#[cfg_attr(
    feature = "c",
    doc = "### C\n```c\nWebhookClientHandle* client = webhook_client_new(config_json, err_buf, sizeof(err_buf));\n```"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webhook_client_new(
    config_json: *const c_char,
    err_buf: *mut c_char,
    err_buf_len: usize,
) -> *mut WebhookClientHandle {
    let json = match unsafe { cstr_to_str(config_json, "config_json") } {
        Ok(s) => s,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return std::ptr::null_mut();
        }
    };

    match WebhookClient::new(json.to_owned()) {
        Ok(client) => Box::into_raw(Box::new(WebhookClientHandle {
            client,
            matchers: std::sync::Mutex::new(Vec::new()),
        })),
        Err(e) => {
            write_err(err_buf, err_buf_len, &e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Send a webhook using an explicit template name and a raw JSON payload string.
///
/// Returns `0` on success; non-zero on error (writes to `err_buf`).
///
/// # Safety
/// See module-level safety contract.
#[cfg_attr(
    feature = "c",
    doc = "### C\n```c\nwebhook_client_send(client, \"alerts\", payload_json, err_buf, sizeof(err_buf));\n```"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webhook_client_send(
    handle: *mut WebhookClientHandle,
    template_name: *const c_char,
    payload_json: *const c_char,
    err_buf: *mut c_char,
    err_buf_len: usize,
) -> c_int {
    if handle.is_null() {
        write_err(err_buf, err_buf_len, "handle must not be NULL");
        return 1;
    }

    let template = match unsafe { cstr_to_str(template_name, "template_name") } {
        Ok(s) => s.to_owned(),
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return 1;
        }
    };

    let payload = match unsafe { cstr_to_str(payload_json, "payload_json") } {
        Ok(s) => s.to_owned(),
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return 1;
        }
    };

    // SAFETY: handle is non-null and was created by webhook_client_new.
    let h = unsafe { &*handle };
    match h.client.send_sync(template, payload) {
        Ok(()) => 0,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e.to_string());
            1
        }
    }
}

/// Walk registered C matcher functions in order; the first non-NULL return value
/// wins. Falls back to the `"default"` template if all matchers return `NULL`.
///
/// Returns `0` on success; non-zero on error (writes to `err_buf`).
///
/// # Safety
/// See module-level safety contract.
#[cfg_attr(
    feature = "c",
    doc = "### C\n```c\nwebhook_client_dispatch(client, payload_json, err_buf, sizeof(err_buf));\n```"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webhook_client_dispatch(
    handle: *mut WebhookClientHandle,
    payload_json: *const c_char,
    err_buf: *mut c_char,
    err_buf_len: usize,
) -> c_int {
    if handle.is_null() {
        write_err(err_buf, err_buf_len, "handle must not be NULL");
        return 1;
    }

    let payload = match unsafe { cstr_to_str(payload_json, "payload_json") } {
        Ok(s) => s.to_owned(),
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return 1;
        }
    };

    // SAFETY: handle is non-null and was created by webhook_client_new.
    let h = unsafe { &*handle };

    // Resolve template via registered C matchers.
    let template_name: String = {
        let matchers = h.matchers.lock().expect("matchers lock poisoned");
        let payload_cstring = match CString::new(payload.as_str()) {
            Ok(cs) => cs,
            Err(e) => {
                write_err(
                    err_buf,
                    err_buf_len,
                    &format!("payload_json contains interior null byte: {e}"),
                );
                return 1;
            }
        };

        let mut resolved = None;
        for matcher_fn in matchers.iter() {
            // SAFETY: matcher_fn is a valid function pointer stored at registration time.
            let result_ptr = unsafe { matcher_fn(payload_cstring.as_ptr()) };
            if !result_ptr.is_null() {
                match unsafe { CStr::from_ptr(result_ptr).to_str() } {
                    Ok(name) => {
                        resolved = Some(name.to_owned());
                        break;
                    }
                    Err(_) => {
                        write_err(
                            err_buf,
                            err_buf_len,
                            "matcher returned non-UTF-8 template name",
                        );
                        return 1;
                    }
                }
            }
        }
        resolved.unwrap_or_else(|| "default".to_owned())
    };

    match h.client.send_sync(template_name, payload) {
        Ok(()) => 0,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e.to_string());
            1
        }
    }
}

/// Register a C matcher function.
///
/// Called once per rule at startup. The function pointer must remain valid
/// for the entire lifetime of the handle. Matchers are consulted in
/// registration order; the first non-NULL return value wins.
///
/// # Safety
/// See module-level safety contract.
#[cfg_attr(
    feature = "c",
    doc = "### C\n```c\nwebhook_client_register_matcher(client, my_matcher_fn);\n```"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webhook_client_register_matcher(
    handle: *mut WebhookClientHandle,
    matcher_fn: Option<WebhookMatcherFn>,
) {
    if handle.is_null() {
        return;
    }
    if let Some(f) = matcher_fn {
        // SAFETY: handle is non-null and was created by webhook_client_new.
        let h = unsafe { &*handle };
        h.matchers.lock().expect("matchers lock poisoned").push(f);
    }
}

/// Flush queued jobs and free the handle.
///
/// Always succeeds. Do not use `handle` after this call.
///
/// # Safety
/// See module-level safety contract.
///
/// Register a new template at runtime.
///
/// Returns 0 on success, or -1 on error.
#[cfg_attr(
    feature = "c",
    doc = "### C\n```c\nwebhook_client_register_template(handle, \"alerts\", \"🚨 {{message}}\", err, sizeof(err));\n```"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webhook_client_register_template(
    handle: *mut WebhookClientHandle,
    name: *const std::os::raw::c_char,
    template: *const std::os::raw::c_char,
    err_buf: *mut std::os::raw::c_char,
    err_buf_len: usize,
) -> std::os::raw::c_int {
    if handle.is_null() {
        write_err(err_buf, err_buf_len, "handle is null");
        return -1;
    }
    let handle = &*handle;

    let name_str = match cstr_to_str(name, "name") {
        Ok(s) => s,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return -1;
        }
    };

    let template_str = match cstr_to_str(template, "template") {
        Ok(s) => s,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return -1;
        }
    };

    match handle
        .client
        .register_template(name_str.to_owned(), template_str.to_owned())
    {
        Ok(_) => 0,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e.to_string());
            -1
        }
    }
}

/// Overwrite an existing template at runtime.
///
/// Returns 0 on success, or -1 on error.
#[cfg_attr(
    feature = "c",
    doc = "### C\n```c\nwebhook_client_update_template(handle, \"alerts\", \"⚠️ {{message}}\", err, sizeof(err));\n```"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webhook_client_update_template(
    handle: *mut WebhookClientHandle,
    name: *const std::os::raw::c_char,
    template: *const std::os::raw::c_char,
    err_buf: *mut std::os::raw::c_char,
    err_buf_len: usize,
) -> std::os::raw::c_int {
    if handle.is_null() {
        write_err(err_buf, err_buf_len, "handle is null");
        return -1;
    }
    let handle = &*handle;

    let name_str = match cstr_to_str(name, "name") {
        Ok(s) => s,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return -1;
        }
    };

    let template_str = match cstr_to_str(template, "template") {
        Ok(s) => s,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return -1;
        }
    };

    match handle
        .client
        .update_template(name_str.to_owned(), template_str.to_owned())
    {
        Ok(_) => 0,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e.to_string());
            -1
        }
    }
}

/// Remove a template at runtime.
#[cfg_attr(
    feature = "c",
    doc = "### C\n```c\nwebhook_client_remove_template(handle, \"alerts\", err, sizeof(err));\n```"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webhook_client_remove_template(
    handle: *mut WebhookClientHandle,
    name: *const std::os::raw::c_char,
    err_buf: *mut std::os::raw::c_char,
    err_buf_len: usize,
) -> std::os::raw::c_int {
    if handle.is_null() {
        write_err(err_buf, err_buf_len, "handle is null");
        return -1;
    }
    let handle = &*handle;

    let name_str = match cstr_to_str(name, "name") {
        Ok(s) => s,
        Err(e) => {
            write_err(err_buf, err_buf_len, &e);
            return -1;
        }
    };

    handle.client.remove_template(name_str.to_owned());
    0
}

#[cfg_attr(feature = "c", doc = "### C\n```c\nwebhook_client_free(client);\n```")]
pub unsafe extern "C" fn webhook_client_free(handle: *mut WebhookClientHandle) {
    if handle.is_null() {
        return;
    }
    // SAFETY: handle was created by Box::into_raw in webhook_client_new.
    let h = unsafe { Box::from_raw(handle) };
    crate::runtime::block_on(h.client.flush());
    // h (and the Arc<WebhookClient> inside) drops here.
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

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

    fn make_handle() -> *mut WebhookClientHandle {
        let config = CString::new(VALID_CONFIG).unwrap();
        let mut err = [0i8; 512];
        unsafe { webhook_client_new(config.as_ptr(), err.as_mut_ptr(), err.len()) }
    }

    fn read_err_buf(buf: &[i8]) -> String {
        let bytes: Vec<u8> = buf.iter().map(|&b| b as u8).collect();
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    // ── webhook_client_new ────────────────────────────────────────────────────

    #[test]
    fn new_valid_config_returns_non_null() {
        let handle = make_handle();
        assert!(!handle.is_null());
        unsafe { webhook_client_free(handle) };
    }

    #[test]
    fn new_invalid_json_returns_null_and_writes_error() {
        let config = CString::new("not json {{{").unwrap();
        let mut err = [0i8; 256];
        let handle = unsafe { webhook_client_new(config.as_ptr(), err.as_mut_ptr(), err.len()) };
        assert!(handle.is_null());
        let msg = read_err_buf(&err);
        assert!(!msg.is_empty(), "expected an error message in err_buf");
    }

    #[test]
    fn new_empty_destinations_returns_null_and_writes_error() {
        let config = CString::new(r#"{"destinations":[]}"#).unwrap();
        let mut err = [0i8; 256];
        let handle = unsafe { webhook_client_new(config.as_ptr(), err.as_mut_ptr(), err.len()) };
        assert!(handle.is_null());
        let msg = read_err_buf(&err);
        assert!(
            msg.contains("at least one destination"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn new_null_config_returns_null_and_writes_error() {
        let mut err = [0i8; 256];
        let handle = unsafe { webhook_client_new(std::ptr::null(), err.as_mut_ptr(), err.len()) };
        assert!(handle.is_null());
        let msg = read_err_buf(&err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn new_err_buf_null_does_not_crash() {
        let config = CString::new("not json").unwrap();
        // null err_buf with 0 len — must not crash
        let handle = unsafe { webhook_client_new(config.as_ptr(), std::ptr::null_mut(), 0) };
        assert!(handle.is_null());
    }

    #[test]
    fn new_err_buf_truncates_to_len() {
        let config = CString::new(r#"{"destinations":[]}"#).unwrap();
        // Only 8 bytes — must not overflow, must be null-terminated within buffer.
        let mut err = [0i8; 8];
        let handle = unsafe { webhook_client_new(config.as_ptr(), err.as_mut_ptr(), err.len()) };
        assert!(handle.is_null());
        // Last byte must still be null (or the whole thing is null-terminated somewhere).
        assert_eq!(err[7], 0, "buffer must be null-terminated");
    }

    // ── webhook_client_send ───────────────────────────────────────────────────

    #[test]
    fn send_null_handle_returns_error() {
        let tmpl = CString::new("default").unwrap();
        let payload = CString::new(r#"{"event":"test","message":"hi"}"#).unwrap();
        let mut err = [0i8; 256];
        let rc = unsafe {
            webhook_client_send(
                std::ptr::null_mut(),
                tmpl.as_ptr(),
                payload.as_ptr(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        assert_ne!(rc, 0);
        let msg = read_err_buf(&err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn send_invalid_payload_json_returns_error() {
        let handle = make_handle();
        assert!(!handle.is_null());
        let tmpl = CString::new("default").unwrap();
        let bad_payload = CString::new("not valid json {{{").unwrap();
        let mut err = [0i8; 256];
        let rc = unsafe {
            webhook_client_send(
                handle,
                tmpl.as_ptr(),
                bad_payload.as_ptr(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        assert_ne!(rc, 0);
        let msg = read_err_buf(&err);
        assert!(!msg.is_empty());
        unsafe { webhook_client_free(handle) };
    }

    // ── webhook_client_dispatch ───────────────────────────────────────────────

    #[test]
    fn dispatch_null_handle_returns_error() {
        let payload = CString::new(r#"{"event":"test","message":"hi"}"#).unwrap();
        let mut err = [0i8; 256];
        let rc = unsafe {
            webhook_client_dispatch(
                std::ptr::null_mut(),
                payload.as_ptr(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        assert_ne!(rc, 0);
    }

    #[test]
    fn dispatch_invalid_payload_json_returns_error() {
        let handle = make_handle();
        assert!(!handle.is_null());
        let bad = CString::new("{bad").unwrap();
        let mut err = [0i8; 256];
        let rc =
            unsafe { webhook_client_dispatch(handle, bad.as_ptr(), err.as_mut_ptr(), err.len()) };
        assert_ne!(rc, 0);
        unsafe { webhook_client_free(handle) };
    }

    // ── webhook_client_register_matcher ──────────────────────────────────────

    #[test]
    fn register_null_handle_does_not_crash() {
        unsafe extern "C" fn my_matcher(_: *const c_char) -> *const c_char {
            std::ptr::null()
        }
        // Must not crash or panic.
        unsafe { webhook_client_register_matcher(std::ptr::null_mut(), Some(my_matcher)) };
    }

    #[test]
    fn register_none_fn_does_not_crash() {
        let handle = make_handle();
        assert!(!handle.is_null());
        unsafe { webhook_client_register_matcher(handle, None) };
        unsafe { webhook_client_free(handle) };
    }

    #[test]
    fn register_matcher_stored_and_counted() {
        unsafe extern "C" fn always_null(_: *const c_char) -> *const c_char {
            std::ptr::null()
        }
        let handle = make_handle();
        assert!(!handle.is_null());
        unsafe {
            webhook_client_register_matcher(handle, Some(always_null));
            webhook_client_register_matcher(handle, Some(always_null));
        }
        // SAFETY: handle is valid and we have exclusive access in this test.
        let count = unsafe { (*handle).matchers.lock().unwrap().len() };
        assert_eq!(count, 2);
        unsafe { webhook_client_free(handle) };
    }

    #[test]
    fn dispatch_matcher_routing_alert_vs_default() {
        // Returns "alerts" when payload contains `"alert"`, NULL otherwise.
        unsafe extern "C" fn alert_matcher(payload: *const c_char) -> *const c_char {
            // SAFETY: payload is a valid null-terminated string for this call.
            let s = unsafe { CStr::from_ptr(payload).to_str().unwrap_or("") };
            if s.contains(r#""alert""#) {
                b"alerts\0".as_ptr() as *const c_char
            } else {
                std::ptr::null()
            }
        }

        let handle = make_handle();
        assert!(!handle.is_null());
        unsafe { webhook_client_register_matcher(handle, Some(alert_matcher)) };

        // Verify routing by inspecting the matchers directly (same logic as dispatch).
        let check = |payload: &str| -> String {
            // SAFETY: handle is valid and we have exclusive access in this test.
            let h = unsafe { &*handle };
            let matchers = h.matchers.lock().unwrap();
            let payload_c = CString::new(payload).unwrap();
            let mut result = None;
            for f in matchers.iter() {
                let ptr = unsafe { f(payload_c.as_ptr()) };
                if !ptr.is_null() {
                    result = Some(unsafe { CStr::from_ptr(ptr).to_str().unwrap().to_owned() });
                    break;
                }
            }
            result.unwrap_or_else(|| "default".to_owned())
        };

        assert_eq!(check(r#"{"level":"alert","message":"down"}"#), "alerts");
        assert_eq!(check(r#"{"level":"info","message":"ok"}"#), "default");

        unsafe { webhook_client_free(handle) };
    }

    // ── webhook_client_free ───────────────────────────────────────────────────

    #[test]
    fn free_null_handle_does_not_crash() {
        unsafe { webhook_client_free(std::ptr::null_mut()) }
    }

    #[test]
    fn test_template_mutations_null_handle() {
        let mut err_buf = [0; 256];
        let name = std::ffi::CString::new("new_temp").unwrap();
        let tmpl = std::ffi::CString::new("hello {{name}}").unwrap();

        let res = unsafe {
            webhook_client_register_template(
                std::ptr::null_mut(),
                name.as_ptr(),
                tmpl.as_ptr(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        assert_eq!(res, -1);
        assert!(read_err_buf(&err_buf).contains("handle is null"));

        let res = unsafe {
            webhook_client_update_template(
                std::ptr::null_mut(),
                name.as_ptr(),
                tmpl.as_ptr(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        assert_eq!(res, -1);

        let res = unsafe {
            webhook_client_remove_template(
                std::ptr::null_mut(),
                name.as_ptr(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        assert_eq!(res, -1);
    }

    #[test]
    fn test_template_mutations() {
        let handle_ptr = make_handle();
        let mut err_buf = [0; 256];

        let name = std::ffi::CString::new("new_temp").unwrap();
        let tmpl = std::ffi::CString::new("hello {{name}}").unwrap();

        let res = unsafe {
            webhook_client_register_template(
                handle_ptr,
                name.as_ptr(),
                tmpl.as_ptr(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        assert_eq!(res, 0);

        let tmpl2 = std::ffi::CString::new("bye {{name}}").unwrap();
        let res = unsafe {
            webhook_client_update_template(
                handle_ptr,
                name.as_ptr(),
                tmpl2.as_ptr(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        assert_eq!(res, 0);

        let res = unsafe {
            webhook_client_remove_template(
                handle_ptr,
                name.as_ptr(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        assert_eq!(res, 0);

        unsafe { webhook_client_free(handle_ptr) };
    }
}
