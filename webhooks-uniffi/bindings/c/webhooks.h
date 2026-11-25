#ifndef WEBHOOKS_H
#define WEBHOOKS_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque handle to a WebhookClient. Never dereference from C.
 * Obtain via webhook_client_new(); release via webhook_client_free().
 */
typedef struct WebhookClientHandle WebhookClientHandle;

/**
 * C matcher function pointer type.
 *
 * Receives the payload JSON as a null-terminated UTF-8 string.
 * Return a null-terminated template name string to select that template,
 * or NULL to pass through to the next registered matcher.
 *
 * The returned pointer must remain valid until the next call to any
 * webhook_client_* function on this handle (i.e. a static string literal
 * is safe; a stack allocation is not).
 */
typedef const char *(*WebhookMatcherFn)(const char *payload_json);

/**
 * Create a new WebhookClient from a JSON config string.
 *
 * Returns NULL on error and writes a null-terminated UTF-8 error message
 * into err_buf (up to err_buf_len bytes including the null terminator).
 * On success returns an opaque handle that must eventually be passed to
 * webhook_client_free().
 *
 * Passing NULL for err_buf (with err_buf_len = 0) is safe when you do not
 * need the error message.
 */
WebhookClientHandle *webhook_client_new(const char *config_json,
                                        char *err_buf,
                                        size_t err_buf_len);

/**
 * Send a webhook using an explicit template name and a raw JSON payload string.
 *
 * Returns 0 on success; non-zero on error (writes to err_buf).
 */
int webhook_client_send(WebhookClientHandle *handle,
                        const char *template_name,
                        const char *payload_json,
                        char *err_buf,
                        size_t err_buf_len);

/**
 * Walk registered C matcher functions in order; the first non-NULL return
 * value wins. Falls back to the "default" template if all matchers return NULL.
 *
 * Returns 0 on success; non-zero on error (writes to err_buf).
 */
int webhook_client_dispatch(WebhookClientHandle *handle,
                            const char *payload_json,
                            char *err_buf,
                            size_t err_buf_len);

/**
 * Register a C matcher function.
 *
 * Called once per rule at startup. The function pointer must remain valid
 * for the entire lifetime of the handle. Matchers are consulted in
 * registration order; the first non-NULL return value wins.
 *
 * Passing NULL for handle or matcher_fn is a no-op.
 */
void webhook_client_register_matcher(WebhookClientHandle *handle,
                                     WebhookMatcherFn matcher_fn);

/**
 * Flush queued jobs and free the handle.
 *
 * Always succeeds. Passing NULL is a no-op.
 * Do not use handle after this call.
 */
int webhook_client_register_template(WebhookClientHandle* handle, const char* name, const char* template_str, char* err_buf, size_t err_buf_len);

int webhook_client_update_template(WebhookClientHandle* handle, const char* name, const char* template_str, char* err_buf, size_t err_buf_len);

int webhook_client_remove_template(WebhookClientHandle* handle, const char* name, char* err_buf, size_t err_buf_len);

void webhook_client_free(WebhookClientHandle* handle);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* WEBHOOKS_H */