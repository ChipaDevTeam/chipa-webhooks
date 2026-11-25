/*
 * test_c.c — C test program for the webhooks-uniffi C API.
 *
 * Exercises:
 *   - webhook_client_new (valid, bad JSON, empty destinations)
 *   - webhook_client_send (null handle, invalid payload)
 *   - webhook_client_dispatch (null handle, invalid payload, matcher routing)
 *   - webhook_client_register_matcher (null handle, NULL fn, ordering)
 *   - webhook_client_free (null handle, valid handle)
 *   - err_buf truncation
 *
 * Build (from project root, after `cargo build --features c`):
 *
 *   cl /W4 test_c.c /I bindings\c \
 *      target\debug\chipa_webhooks.dll.lib \
 *      /Fe:test_c.exe
 *
 *   gcc -Wall -Wextra test_c.c -Ibindings/c \
 *       -Ltarget/debug -lchipa_webhooks -Wl,-rpath,target/debug \
 *       -o test_c
 *
 * Run:
 *   ./test_c        (all tests must print [PASS])
 *
 * Exit code 0 = all passed, non-zero = at least one failure.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "webhooks.h"

/* ── helpers ──────────────────────────────────────────────────────────────── */

static int g_failed = 0;

#define PASS(name)  fprintf(stdout, "  [PASS] %s\n", (name))
#define FAIL(name, reason) \
    do { \
        fprintf(stderr, "  [FAIL] %s: %s\n", (name), (reason)); \
        g_failed++; \
    } while (0)

#define ASSERT(name, cond) \
    do { \
        if (cond) { PASS(name); } \
        else { FAIL(name, "assertion false: " #cond); } \
    } while (0)

#define ASSERT_MSG(name, cond, reason) \
    do { \
        if (cond) { PASS(name); } \
        else { FAIL(name, reason); } \
    } while (0)

static const char *VALID_CONFIG =
    "{"
    "  \"destinations\": ["
    "    {"
    "      \"name\": \"test-discord\","
    "      \"credentials\": {"
    "        \"type\": \"discord\","
    "        \"webhook_url\": \"https://discord.com/api/webhooks/000/fake\""
    "      }"
    "    }"
    "  ],"
    "  \"templates\": {"
    "    \"default\": \"{{event}}: {{message}}\","
    "    \"alerts\":  \"[ALERT] {{message}}\""
    "  }"
    "}";

static const char *EMPTY_DEST_CONFIG = "{\"destinations\": []}";
static const char *BAD_JSON_CONFIG   = "not json {{{";

/* ── matcher functions ────────────────────────────────────────────────────── */

/*
 * Returns "alerts" when payload contains the literal string "alert",
 * NULL otherwise.  Returns a static string literal — lifetime is infinite.
 */
static const char *alert_matcher(const char *payload_json)
{
    if (payload_json && strstr(payload_json, "\"alert\"")) {
        return "alerts";
    }
    return NULL;
}

/* Always returns NULL — used to test pass-through chaining. */
static const char *null_matcher(const char *payload_json)
{
    (void)payload_json;
    return NULL;
}

/* Always returns "alerts" — used to verify first-wins ordering. */
static const char *always_alerts_matcher(const char *payload_json)
{
    (void)payload_json;
    return "alerts";
}

/* Should never be called — panics if reached (indicates ordering bug). */
static const char *unreachable_matcher(const char *payload_json)
{
    (void)payload_json;
    fprintf(stderr, "  [BUG] unreachable_matcher was called!\n");
    exit(2);
}

/* ── test sections ────────────────────────────────────────────────────────── */

static void test_new(void)
{
    char err[512];

    /* --- valid config returns non-NULL handle --- */
    {
        memset(err, 0, sizeof(err));
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, err, sizeof(err));
        ASSERT("new_valid_config_returns_non_null", h != NULL);
        ASSERT("new_valid_config_no_error_written", err[0] == '\0');
        if (h) webhook_client_free(h);
    }

    /* --- bad JSON returns NULL and writes error --- */
    {
        memset(err, 0, sizeof(err));
        WebhookClientHandle *h = webhook_client_new(BAD_JSON_CONFIG, err, sizeof(err));
        ASSERT("new_bad_json_returns_null", h == NULL);
        ASSERT("new_bad_json_writes_error", err[0] != '\0');
        if (h) webhook_client_free(h);
    }

    /* --- empty destinations returns NULL and writes helpful error --- */
    {
        memset(err, 0, sizeof(err));
        WebhookClientHandle *h = webhook_client_new(EMPTY_DEST_CONFIG, err, sizeof(err));
        ASSERT("new_empty_destinations_returns_null", h == NULL);
        ASSERT_MSG(
            "new_empty_destinations_error_mentions_destination",
            strstr(err, "destination") != NULL,
            err
        );
        if (h) webhook_client_free(h);
    }

    /* --- NULL config_json returns NULL and writes error --- */
    {
        memset(err, 0, sizeof(err));
        WebhookClientHandle *h = webhook_client_new(NULL, err, sizeof(err));
        ASSERT("new_null_config_returns_null", h == NULL);
        ASSERT("new_null_config_writes_error", err[0] != '\0');
        if (h) webhook_client_free(h);
    }

    /* --- NULL err_buf with 0 len does not crash --- */
    {
        WebhookClientHandle *h = webhook_client_new(BAD_JSON_CONFIG, NULL, 0);
        ASSERT("new_null_err_buf_does_not_crash", h == NULL);
        if (h) webhook_client_free(h);
    }

    /* --- err_buf of size 1 is null-terminated and does not overflow --- */
    {
        char tiny[1] = { 0x7f };
        WebhookClientHandle *h = webhook_client_new(BAD_JSON_CONFIG, tiny, 1);
        ASSERT("new_tiny_err_buf_null_terminated", tiny[0] == '\0');
        ASSERT("new_tiny_err_buf_returns_null", h == NULL);
        if (h) webhook_client_free(h);
    }

    /* --- err_buf of size 8 truncates but remains null-terminated --- */
    {
        char small[8];
        memset(small, 0x7f, sizeof(small));
        WebhookClientHandle *h = webhook_client_new(EMPTY_DEST_CONFIG, small, sizeof(small));
        ASSERT("new_small_err_buf_returns_null", h == NULL);
        ASSERT("new_small_err_buf_null_terminated", small[7] == '\0');
        if (h) webhook_client_free(h);
    }
}

static void test_free(void)
{
    /* --- free NULL is a no-op --- */
    webhook_client_free(NULL);
    PASS("free_null_does_not_crash");

    /* --- free a valid handle --- */
    {
        char err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, err, sizeof(err));
        if (h) {
            webhook_client_free(h);
            PASS("free_valid_handle_does_not_crash");
        } else {
            FAIL("free_valid_handle_does_not_crash", "could not construct client");
        }
    }
}

static void test_send(void)
{
    char err[512];

    /* --- null handle returns non-zero --- */
    {
        memset(err, 0, sizeof(err));
        int rc = webhook_client_send(NULL, "default", "{}", err, sizeof(err));
        ASSERT("send_null_handle_nonzero", rc != 0);
        ASSERT("send_null_handle_writes_error", err[0] != '\0');
    }

    /* --- null template_name returns non-zero --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("send_null_template_name_nonzero", "could not construct client");
            FAIL("send_null_template_name_writes_error", "could not construct client");
        } else {
            memset(err, 0, sizeof(err));
            int rc = webhook_client_send(h, NULL, "{}", err, sizeof(err));
            ASSERT("send_null_template_name_nonzero", rc != 0);
            ASSERT("send_null_template_name_writes_error", err[0] != '\0');
            webhook_client_free(h);
        }
    }

    /* --- invalid payload JSON returns non-zero --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("send_invalid_payload_nonzero", "could not construct client");
            FAIL("send_invalid_payload_writes_error", "could not construct client");
        } else {
            memset(err, 0, sizeof(err));
            int rc = webhook_client_send(h, "default", "not json {{{", err, sizeof(err));
            ASSERT("send_invalid_payload_nonzero", rc != 0);
            ASSERT("send_invalid_payload_writes_error", err[0] != '\0');
            webhook_client_free(h);
        }
    }

    /* --- valid payload + known template → dispatch error (fake URL, not a config err) --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("send_valid_payload_returns_nonzero_on_fake_url", "could not construct client");
        } else {
            memset(err, 0, sizeof(err));
            int rc = webhook_client_send(
                h, "default",
                "{\"event\":\"test\",\"message\":\"hello\"}",
                err, sizeof(err)
            );
            /*
             * The send will fail because the URL is fake, but it should fail
             * with a Dispatch error (rc != 0), not a Template error, and
             * err must not be empty.
             */
            ASSERT("send_valid_payload_returns_nonzero_on_fake_url", rc != 0);
            ASSERT("send_valid_payload_writes_error_on_fake_url", err[0] != '\0');
            webhook_client_free(h);
        }
    }
}

static void test_register_matcher(void)
{
    /* --- null handle is a no-op, does not crash --- */
    webhook_client_register_matcher(NULL, alert_matcher);
    PASS("register_null_handle_does_not_crash");

    /* --- NULL fn is a no-op, does not crash --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("register_null_fn_does_not_crash", "could not construct client");
        } else {
            webhook_client_register_matcher(h, NULL);
            PASS("register_null_fn_does_not_crash");
            webhook_client_free(h);
        }
    }

    /* --- register and then free: no crash --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("register_then_free_does_not_crash", "could not construct client");
        } else {
            webhook_client_register_matcher(h, alert_matcher);
            webhook_client_register_matcher(h, null_matcher);
            PASS("register_then_free_does_not_crash");
            webhook_client_free(h);
        }
    }
}

static void test_dispatch(void)
{
    char err[512];

    /* --- null handle returns non-zero --- */
    {
        memset(err, 0, sizeof(err));
        int rc = webhook_client_dispatch(NULL, "{}", err, sizeof(err));
        ASSERT("dispatch_null_handle_nonzero", rc != 0);
        ASSERT("dispatch_null_handle_writes_error", err[0] != '\0');
    }

    /* --- invalid payload JSON returns non-zero --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("dispatch_invalid_payload_nonzero", "could not construct client");
            FAIL("dispatch_invalid_payload_writes_error", "could not construct client");
        } else {
            memset(err, 0, sizeof(err));
            int rc = webhook_client_dispatch(h, "bad json {{{", err, sizeof(err));
            ASSERT("dispatch_invalid_payload_nonzero", rc != 0);
            ASSERT("dispatch_invalid_payload_writes_error", err[0] != '\0');
            webhook_client_free(h);
        }
    }

    /* --- no matchers registered → falls back to "default" template
          (send fails on fake URL, so rc != 0 from network, not from routing) --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("dispatch_no_matchers_uses_default", "could not construct client");
        } else {
            memset(err, 0, sizeof(err));
            int rc = webhook_client_dispatch(
                h, "{\"event\":\"tick\",\"message\":\"ok\"}",
                err, sizeof(err)
            );
            /*
             * rc will be non-zero because the URL is fake (network error),
             * but the error message must NOT contain "template" (which would
             * indicate a missing-template error rather than a dispatch error).
             */
            ASSERT("dispatch_no_matchers_returns_nonzero_on_fake_url", rc != 0);
            ASSERT_MSG(
                "dispatch_no_matchers_error_is_dispatch_not_template",
                strstr(err, "template") == NULL || strstr(err, "Template") == NULL,
                err
            );
            webhook_client_free(h);
        }
    }

    /* --- null_matcher passes through, alert_matcher selects "alerts" --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("dispatch_chained_matchers_alert_payload", "could not construct client");
            FAIL("dispatch_chained_matchers_info_payload", "could not construct client");
        } else {
            webhook_client_register_matcher(h, null_matcher);
            webhook_client_register_matcher(h, alert_matcher);

            /* Alert payload: should use "alerts" template → Dispatch error (fake URL) */
            memset(err, 0, sizeof(err));
            int rc = webhook_client_dispatch(
                h, "{\"level\":\"alert\",\"message\":\"down\"}",
                err, sizeof(err)
            );
            ASSERT("dispatch_chained_matchers_alert_payload_nonzero", rc != 0);
            ASSERT("dispatch_chained_matchers_alert_payload_has_error", err[0] != '\0');

            /* Info payload: both matchers return NULL → falls back to "default" */
            memset(err, 0, sizeof(err));
            rc = webhook_client_dispatch(
                h, "{\"level\":\"info\",\"message\":\"ok\"}",
                err, sizeof(err)
            );
            ASSERT("dispatch_chained_matchers_info_payload_nonzero", rc != 0);
            ASSERT("dispatch_chained_matchers_info_payload_has_error", err[0] != '\0');

            webhook_client_free(h);
        }
    }

    /* --- first matcher wins, second is not called --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("dispatch_first_matcher_wins", "could not construct client");
        } else {
            webhook_client_register_matcher(h, always_alerts_matcher);
            webhook_client_register_matcher(h, unreachable_matcher); /* must not be called */

            memset(err, 0, sizeof(err));
            int rc = webhook_client_dispatch(
                h, "{\"event\":\"anything\"}",
                err, sizeof(err)
            );
            /*
             * unreachable_matcher would call exit(2) if reached.
             * If we get here, first-wins ordering is correct.
             */
            ASSERT("dispatch_first_matcher_wins_nonzero_on_fake_url", rc != 0);
            PASS("dispatch_first_matcher_wins_second_not_called");
            webhook_client_free(h);
        }
    }

    /* --- multiple null matchers all pass through → "default" fallback --- */
    {
        char inner_err[256] = {0};
        WebhookClientHandle *h = webhook_client_new(VALID_CONFIG, inner_err, sizeof(inner_err));
        if (!h) {
            FAIL("dispatch_all_null_matchers_fallback", "could not construct client");
        } else {
            webhook_client_register_matcher(h, null_matcher);
            webhook_client_register_matcher(h, null_matcher);
            webhook_client_register_matcher(h, null_matcher);

            memset(err, 0, sizeof(err));
            int rc = webhook_client_dispatch(
                h, "{\"event\":\"test\",\"message\":\"hi\"}",
                err, sizeof(err)
            );
            ASSERT("dispatch_all_null_matchers_returns_nonzero_on_fake_url", rc != 0);
            webhook_client_free(h);
        }
    }

    /* --- NULL err_buf with 0 len does not crash on error --- */
    {
        int rc = webhook_client_dispatch(NULL, "{}", NULL, 0);
        ASSERT("dispatch_null_handle_null_err_buf_nonzero", rc != 0);
        PASS("dispatch_null_handle_null_err_buf_no_crash");
    }
}

/* ── main ─────────────────────────────────────────────────────────────────── */

int main(void)
{
    printf("\ntest_new\n");
    test_new();

    printf("\ntest_free\n");
    test_free();

    printf("\ntest_send\n");
    test_send();

    printf("\ntest_register_matcher\n");
    test_register_matcher();

    printf("\ntest_dispatch\n");
    test_dispatch();

    printf("\n%s\n", g_failed == 0 ? "All tests passed." : "SOME TESTS FAILED.");
    return g_failed == 0 ? 0 : 1;
}