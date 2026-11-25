"""
Pytest test suite for the chipa-webhooks UniFFI Python bindings.

Covers:
- Construction validation (valid, invalid JSON, empty destinations, duplicates, unknown platform)
- Error variant types
- Matcher registration and callback interface
- dispatch_sync routing (matcher wins vs falls back to "default")
- send_sync with bad payload
- shutdown_sync
"""

import json
import os
import sys

import pytest

# Allow running from the repo root or from the bindings directory.
sys.path.insert(0, os.path.join(os.path.dirname(__file__)))

import chipa_webhooks as w

# ---------------------------------------------------------------------------
# Shared fixtures
# ---------------------------------------------------------------------------

DISCORD_CONFIG = json.dumps(
    {
        "destinations": [
            {
                "name": "test-discord",
                "credentials": {
                    "type": "discord",
                    "webhook_url": "https://discord.com/api/webhooks/000/fake",
                },
            }
        ],
        "templates": {
            "default": "{{event}}: {{message}}",
            "alerts": "[ALERT] {{message}}",
        },
    }
)

MULTI_DEST_CONFIG = json.dumps(
    {
        "destinations": [
            {
                "name": "discord",
                "credentials": {
                    "type": "discord",
                    "webhook_url": "https://discord.com/api/webhooks/000/fake",
                },
            },
            {
                "name": "telegram",
                "credentials": {
                    "type": "telegram",
                    "token": "bot:fake",
                    "chat_id": -1001234567890,
                },
            },
        ],
        "templates": {
            "default": "{{message}}",
            "critical": "CRITICAL: {{message}}",
        },
        "capacity": 512,
    }
)


@pytest.fixture
def client():
    """A valid WebhookClient built from DISCORD_CONFIG."""
    return w.WebhookClient(DISCORD_CONFIG)


@pytest.fixture
def multi_client():
    """A valid WebhookClient with multiple destinations."""
    return w.WebhookClient(MULTI_DEST_CONFIG)


# ---------------------------------------------------------------------------
# Importability
# ---------------------------------------------------------------------------


class TestImports:
    def test_webhook_client_importable(self):
        assert hasattr(w, "WebhookClient")

    def test_webhook_ffi_error_importable(self):
        assert hasattr(w, "WebhookFfiError")

    def test_matcher_callback_importable(self):
        assert hasattr(w, "MatcherCallback")

    def test_error_variants_exist(self):
        assert hasattr(w.WebhookFfiError, "Config")
        assert hasattr(w.WebhookFfiError, "Dispatch")
        assert hasattr(w.WebhookFfiError, "Template")
        assert hasattr(w.WebhookFfiError, "Internal")


# ---------------------------------------------------------------------------
# Construction — valid configs
# ---------------------------------------------------------------------------


class TestConstructionValid:
    def test_single_discord_destination(self):
        client = w.WebhookClient(DISCORD_CONFIG)
        assert client is not None

    def test_multi_destination_config(self):
        client = w.WebhookClient(MULTI_DEST_CONFIG)
        assert client is not None

    def test_slack_destination(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "slack",
                        "credentials": {
                            "type": "slack",
                            "webhook_url": "https://hooks.slack.com/services/fake",
                        },
                    }
                ]
            }
        )
        client = w.WebhookClient(config)
        assert client is not None

    def test_ntfy_destination(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "ntfy",
                        "credentials": {
                            "type": "ntfy",
                            "url": "https://ntfy.sh",
                            "topic": "test",
                        },
                    }
                ]
            }
        )
        client = w.WebhookClient(config)
        assert client is not None

    def test_generic_destination(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "generic",
                        "credentials": {
                            "type": "generic",
                            "url": "https://webhook.site/fake-uuid",
                        },
                    }
                ]
            }
        )
        client = w.WebhookClient(config)
        assert client is not None

    def test_telegram_destination(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "tg",
                        "credentials": {
                            "type": "telegram",
                            "token": "1234:fake",
                            "chat_id": -100123456789,
                        },
                    }
                ]
            }
        )
        client = w.WebhookClient(config)
        assert client is not None

    def test_optional_capacity_omitted(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "d",
                        "credentials": {
                            "type": "discord",
                            "webhook_url": "https://discord.com/api/webhooks/0/x",
                        },
                    }
                ]
            }
        )
        client = w.WebhookClient(config)
        assert client is not None

    def test_explicit_capacity(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "d",
                        "credentials": {
                            "type": "discord",
                            "webhook_url": "https://discord.com/api/webhooks/0/x",
                        },
                    }
                ],
                "capacity": 2048,
            }
        )
        client = w.WebhookClient(config)
        assert client is not None


# ---------------------------------------------------------------------------
# Construction — invalid configs → WebhookFfiError.Config
# ---------------------------------------------------------------------------


class TestConstructionErrors:
    def _assert_config_error(self, config_str, substring=None):
        with pytest.raises(w.WebhookFfiError.Config) as exc_info:
            w.WebhookClient(config_str)
        if substring is not None:
            assert substring in str(exc_info.value), (
                f"Expected {substring!r} in error message, got: {exc_info.value}"
            )

    def test_invalid_json(self):
        self._assert_config_error("not json at all {{{")

    def test_empty_string(self):
        self._assert_config_error("")

    def test_empty_destinations_list(self):
        self._assert_config_error(
            json.dumps({"destinations": []}),
            "at least one destination",
        )

    def test_missing_destinations_field(self):
        self._assert_config_error(json.dumps({"templates": {"default": "hi"}}))

    def test_duplicate_destination_names(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "dup",
                        "credentials": {
                            "type": "discord",
                            "webhook_url": "https://a",
                        },
                    },
                    {
                        "name": "dup",
                        "credentials": {
                            "type": "discord",
                            "webhook_url": "https://b",
                        },
                    },
                ]
            }
        )
        self._assert_config_error(config, "dup")

    def test_unknown_platform_type(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "x",
                        "credentials": {
                            "type": "carrier_pigeon",
                            "url": "https://a",
                        },
                    }
                ]
            }
        )
        self._assert_config_error(config)

    def test_missing_required_field_in_credentials(self):
        # Discord requires webhook_url; omit it.
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "d",
                        "credentials": {"type": "discord"},
                    }
                ]
            }
        )
        self._assert_config_error(config)

    def test_null_destinations_value(self):
        self._assert_config_error(json.dumps({"destinations": None}))

    def test_wrong_type_for_destinations(self):
        self._assert_config_error(json.dumps({"destinations": "not-a-list"}))


# ---------------------------------------------------------------------------
# Matcher registration
# ---------------------------------------------------------------------------


class TestMatcherRegistration:
    def test_register_single_matcher(self, client):
        class MyMatcher(w.MatcherCallback):
            def match_template(self, payload_json: str):
                return None

        client.register_matcher(MyMatcher())  # must not raise

    def test_register_multiple_matchers(self, client):
        class M(w.MatcherCallback):
            def match_template(self, payload_json: str):
                return None

        client.register_matcher(M())
        client.register_matcher(M())
        client.register_matcher(M())

    def test_matcher_receives_payload_string(self, client):
        received = []

        class CaptureMatcher(w.MatcherCallback):
            def match_template(self, payload_json: str):
                received.append(payload_json)
                return None

        client.register_matcher(CaptureMatcher())

        # dispatch_sync will call the matcher; the send will fail (fake URL)
        # but we only care that the matcher was called with valid JSON.
        try:
            client.dispatch_sync(json.dumps({"event": "probe"}))
        except w.WebhookFfiError.Dispatch:
            pass

        assert len(received) == 1
        parsed = json.loads(received[0])
        assert parsed["event"] == "probe"

    def test_matcher_returns_none_passes_through(self, client):
        call_count = [0]

        class PassMatcher(w.MatcherCallback):
            def match_template(self, payload_json: str):
                call_count[0] += 1
                return None

        class FinalMatcher(w.MatcherCallback):
            def match_template(self, payload_json: str):
                call_count[0] += 1
                return "alerts"

        client.register_matcher(PassMatcher())
        client.register_matcher(FinalMatcher())

        # Both matchers should be called (first passes, second wins).
        try:
            client.dispatch_sync(json.dumps({"msg": "test"}))
        except w.WebhookFfiError.Dispatch:
            pass

        assert call_count[0] == 2

    def test_first_matcher_wins_second_not_called(self, client):
        call_log = []

        class WinsMatcher(w.MatcherCallback):
            def match_template(self, payload_json: str):
                call_log.append("first")
                return "alerts"

        class ShouldNotBeCalled(w.MatcherCallback):
            def match_template(self, payload_json: str):
                call_log.append("second")
                return "default"

        client.register_matcher(WinsMatcher())
        client.register_matcher(ShouldNotBeCalled())

        try:
            client.dispatch_sync(json.dumps({"msg": "test"}))
        except w.WebhookFfiError.Dispatch:
            pass

        assert call_log == ["first"], (
            f"Only first matcher should be called, got: {call_log}"
        )

    def test_no_matchers_uses_default_template(self, client):
        # No matchers registered — dispatch_sync falls back to "default".
        # The send will fail (fake URL) but we should get a Dispatch error,
        # not a Template error (which would mean "default" template not found).
        with pytest.raises(w.WebhookFfiError.Dispatch):
            client.dispatch_sync(json.dumps({"event": "test", "message": "hi"}))

    def test_all_matchers_return_none_falls_back_to_default(self, client):
        class NullMatcher(w.MatcherCallback):
            def match_template(self, payload_json: str):
                return None

        client.register_matcher(NullMatcher())
        client.register_matcher(NullMatcher())

        # Should still fall back to "default" template → Dispatch error (not Template).
        with pytest.raises(w.WebhookFfiError.Dispatch):
            client.dispatch_sync(json.dumps({"event": "test", "message": "hi"}))

    def test_matcher_selects_alerts_template(self, client):
        class AlertMatcher(w.MatcherCallback):
            def match_template(self, payload_json: str):
                try:
                    data = json.loads(payload_json)
                    if data.get("level") == "alert":
                        return "alerts"
                except Exception:
                    pass
                return None

        client.register_matcher(AlertMatcher())

        # Both routes lead to Dispatch (fake URL) not Template (template exists).
        with pytest.raises(w.WebhookFfiError.Dispatch):
            client.dispatch_sync(json.dumps({"level": "alert", "message": "down"}))

        with pytest.raises(w.WebhookFfiError.Dispatch):
            client.dispatch_sync(json.dumps({"level": "info", "message": "ok"}))


# ---------------------------------------------------------------------------
# send_sync error paths
# ---------------------------------------------------------------------------


class TestSendSync:
    def test_invalid_payload_json_raises_config_error(self, client):
        with pytest.raises(w.WebhookFfiError.Config):
            client.send_sync("default", "not valid json {{{{")

    def test_empty_payload_raises_config_error(self, client):
        with pytest.raises(w.WebhookFfiError.Config):
            client.send_sync("default", "")

    def test_valid_payload_raises_dispatch_on_fake_url(self, client):
        # The URL is fake so we expect a network error, not a config/template error.
        with pytest.raises(w.WebhookFfiError.Dispatch):
            client.send_sync("default", json.dumps({"event": "test", "message": "hi"}))

    def test_valid_payload_wrong_template_name(self, client):
        # "nonexistent" template is not registered → Template error.
        with pytest.raises(w.WebhookFfiError.Template):
            client.send_sync(
                "nonexistent_template_xyz",
                json.dumps({"event": "test", "message": "hi"}),
            )


# ---------------------------------------------------------------------------
# dispatch_sync error paths
# ---------------------------------------------------------------------------


class TestDispatchSync:
    def test_invalid_payload_json_raises_config_error(self, client):
        with pytest.raises(w.WebhookFfiError.Config):
            client.dispatch_sync("{bad json")

    def test_empty_payload_raises_config_error(self, client):
        with pytest.raises(w.WebhookFfiError.Config):
            client.dispatch_sync("")

    def test_valid_payload_raises_dispatch_on_fake_url(self, client):
        with pytest.raises(w.WebhookFfiError.Dispatch):
            client.dispatch_sync(json.dumps({"event": "test", "message": "hi"}))


# ---------------------------------------------------------------------------
# shutdown_sync
# ---------------------------------------------------------------------------


class TestShutdownSync:
    def test_shutdown_sync_does_not_raise(self, client):
        client.shutdown_sync()

    def test_shutdown_sync_on_fresh_client(self):
        c = w.WebhookClient(DISCORD_CONFIG)
        c.shutdown_sync()

    def test_shutdown_sync_after_failed_send(self, client):
        try:
            client.send_sync("default", json.dumps({"event": "x", "message": "y"}))
        except w.WebhookFfiError.Dispatch:
            pass
        client.shutdown_sync()


# ---------------------------------------------------------------------------
# Error message quality
# ---------------------------------------------------------------------------


class TestErrorMessages:
    def test_empty_destinations_message_is_helpful(self):
        try:
            w.WebhookClient(json.dumps({"destinations": []}))
            pytest.fail("expected Config error")
        except w.WebhookFfiError.Config as e:
            msg = str(e)
            assert "destination" in msg.lower(), (
                f"Error should mention 'destination': {msg}"
            )

    def test_duplicate_name_message_contains_name(self):
        config = json.dumps(
            {
                "destinations": [
                    {
                        "name": "my-unique-dup-name",
                        "credentials": {
                            "type": "discord",
                            "webhook_url": "https://a",
                        },
                    },
                    {
                        "name": "my-unique-dup-name",
                        "credentials": {
                            "type": "discord",
                            "webhook_url": "https://b",
                        },
                    },
                ]
            }
        )
        try:
            w.WebhookClient(config)
            pytest.fail("expected Config error")
        except w.WebhookFfiError.Config as e:
            assert "my-unique-dup-name" in str(e)

    def test_invalid_json_payload_message_mentions_json(self, client):
        try:
            client.send_sync("default", "clearly not json")
            pytest.fail("expected Config error")
        except w.WebhookFfiError.Config as e:
            # The error should give the user a clue it's a JSON parse problem.
            msg = str(e).lower()
            assert "json" in msg or "parse" in msg or "payload" in msg, (
                f"Error message should mention JSON/parse/payload: {e}"
            )


# ---------------------------------------------------------------------------
# Multi-destination client smoke tests
# ---------------------------------------------------------------------------


class TestMultiDestination:
    def test_construction_succeeds(self, multi_client):
        assert multi_client is not None

    def test_dispatch_raises_dispatch_error(self, multi_client):
        with pytest.raises(w.WebhookFfiError.Dispatch):
            multi_client.dispatch_sync(json.dumps({"message": "hi"}))

    def test_send_to_existing_template(self, multi_client):
        with pytest.raises(w.WebhookFfiError.Dispatch):
            multi_client.send_sync("critical", json.dumps({"message": "hi"}))

    def test_send_to_missing_template(self, multi_client):
        with pytest.raises(w.WebhookFfiError.Template):
            multi_client.send_sync("does_not_exist", json.dumps({"message": "hi"}))
