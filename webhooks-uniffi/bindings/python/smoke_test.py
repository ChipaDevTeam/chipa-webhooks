import sys

sys.path.insert(0, "bindings/python")

import chipa_webhooks as w

DISCORD_CONFIG = """{
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
}"""


def test_import():
    assert hasattr(w, "WebhookClient")
    assert hasattr(w, "WebhookFfiError")
    print("  [ok] WebhookClient and WebhookFfiError are importable")


def test_construction_valid():
    client = w.WebhookClient(DISCORD_CONFIG)
    assert client is not None
    print("  [ok] WebhookClient constructed from valid config")


def test_error_empty_destinations():
    bad = '{"destinations": []}'
    try:
        w.WebhookClient(bad)
        raise AssertionError("expected WebhookFfiError.Config, got nothing")
    except w.WebhookFfiError.Config as e:
        assert "at least one destination" in str(e)
        print(f"  [ok] empty destinations raises Config error: {e}")


def test_error_missing_destinations_field():
    bad = '{"templates": {"default": "hi"}}'
    try:
        w.WebhookClient(bad)
        raise AssertionError("expected WebhookFfiError.Config, got nothing")
    except w.WebhookFfiError.Config as e:
        print(f"  [ok] missing destinations field raises Config error: {e}")


def test_error_duplicate_destination_names():
    bad = """{
        "destinations": [
            {"name": "dup", "credentials": {"type": "discord", "webhook_url": "https://a"}},
            {"name": "dup", "credentials": {"type": "discord", "webhook_url": "https://b"}}
        ]
    }"""
    try:
        w.WebhookClient(bad)
        raise AssertionError("expected WebhookFfiError.Config, got nothing")
    except w.WebhookFfiError.Config as e:
        assert "dup" in str(e)
        print(f"  [ok] duplicate names raises Config error: {e}")


def test_error_unknown_platform():
    bad = """{
        "destinations": [
            {"name": "x", "credentials": {"type": "carrier_pigeon", "url": "https://a"}}
        ]
    }"""
    try:
        w.WebhookClient(bad)
        raise AssertionError("expected WebhookFfiError.Config, got nothing")
    except w.WebhookFfiError.Config as e:
        print(f"  [ok] unknown platform raises Config error: {e}")


def test_error_invalid_json():
    try:
        w.WebhookClient("not json at all {{{")
        raise AssertionError("expected WebhookFfiError.Config, got nothing")
    except w.WebhookFfiError.Config as e:
        print(f"  [ok] invalid JSON raises Config error: {e}")


def test_matcher_registration():
    client = w.WebhookClient(DISCORD_CONFIG)

    class AlertMatcher(w.MatcherCallback):
        def match_template(self, payload_json: str):
            import json

            try:
                data = json.loads(payload_json)
                if data.get("level") == "alert":
                    return "alerts"
            except Exception:
                pass
            return None

    client.register_matcher(AlertMatcher())
    print("  [ok] matcher registered without error")


def test_shutdown_sync():
    client = w.WebhookClient(DISCORD_CONFIG)
    client.shutdown_sync()
    print("  [ok] shutdown_sync completed without error")


TESTS = [
    test_import,
    test_construction_valid,
    test_error_empty_destinations,
    test_error_missing_destinations_field,
    test_error_duplicate_destination_names,
    test_error_unknown_platform,
    test_error_invalid_json,
    test_matcher_registration,
    test_shutdown_sync,
]

if __name__ == "__main__":
    failed = 0
    for t in TESTS:
        name = t.__name__
        try:
            print(f"\n{name}")
            t()
        except Exception as e:
            print(f"  [FAIL] {e}")
            failed += 1

    print(f"\n{'=' * 50}")
    print(f"Results: {len(TESTS) - failed}/{len(TESTS)} passed")
    if failed:
        sys.exit(1)
