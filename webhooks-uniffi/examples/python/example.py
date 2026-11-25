import asyncio
import json
import sys

try:
    # Import the generated UniFFI Python bindings.
    # Make sure you've generated them using `uniffi-bindgen` and they are in your PYTHONPATH.
    import chipa_webhooks
except ImportError:
    print("Error: Could not import chipa_webhooks.")
    print(
        "Ensure you have generated the Python bindings and they are in your PYTHONPATH."
    )
    sys.exit(1)


# 1. Define a custom Matcher (Rule)
# This intercepts `dispatch` calls and decides which template to use based on the payload.
# If it returns None, it falls back to the next matcher or "default".
class AlertMatcher(chipa_webhooks.MatcherCallback):
    def match_template(self, payload_json: str) -> str | None:
        try:
            # The send/dispatch methods take a raw JSON string.
            # Here we parse it in Python to inspect the contents.
            data = json.loads(payload_json)
            if data.get("level") == "critical":
                return "alerts"
        except json.JSONDecodeError:
            pass
        return None


async def main():
    print("--- Chipa Webhooks Python Example ---")

    # 2. Configuration with Multiple Destinations and Templates
    # The client takes a JSON string config.
    config = {
        "destinations": [
            {
                "name": "my-discord",
                "credentials": {
                    "type": "discord",
                    "webhook_url": "https://discord.com/api/webhooks/0000/fake-token",
                },
            },
            {
                "name": "my-telegram",
                "credentials": {
                    "type": "telegram",
                    "token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11",
                    "chat_id": -100123456789,
                },
            },
        ],
        "templates": {
            "default": "New event: {{message}}",
            "alerts": "🚨 [ALERT] 🚨 {{message}}",
        },
    }

    # Initialize the client
    client = chipa_webhooks.WebhookClient(json.dumps(config))

    # 3. Register the custom rule
    client.register_matcher(AlertMatcher())

    # 4. Sending Sync Messages
    print("\n[Sync] Sending direct message using 'default' template...")
    # The `send` and `dispatch` methods take a raw JSON string.
    # The Rust core will parse this string into a serde_json::Value to render the Handlebars template.
    client.send_sync("default", json.dumps({"message": "Hello from Sync Send!"}))

    print("[Sync] Dispatching a critical alert...")
    # This will be routed to the 'alerts' template by our AlertMatcher.
    client.dispatch_sync(
        json.dumps({"level": "critical", "message": "The database is down!"})
    )

    print("[Sync] Dispatching a normal info event...")
    # This won't match our AlertMatcher, so it falls back to the 'default' template.
    client.dispatch_sync(
        json.dumps({"level": "info", "message": "Routine backup completed."})
    )

    # 5. Sending Async Messages
    print("\n[Async] Sending direct message using 'alerts' template...")
    await client.send("alerts", json.dumps({"message": "Async system warning!"}))

    print("[Async] Dispatching a critical alert...")
    await client.dispatch(
        json.dumps({"level": "critical", "message": "CPU usage at 99%!"})
    )

    # 6. Runtime Template Mutations
    print("\n[Mutations] Registering a new runtime template...")
    client.register_template("runtime_test", "This is a runtime template: {{message}}")
    client.send_sync("runtime_test", json.dumps({"message": "Hello Runtime!"}))

    print("[Mutations] Updating an existing template...")
    client.update_template("runtime_test", "UPDATED runtime template: {{message}}")
    client.send_sync("runtime_test", json.dumps({"message": "Hello again!"}))

    print("[Mutations] Removing the template...")
    client.remove_template("runtime_test")

    # 7. Graceful Shutdown
    # This flushes any pending HTTP requests in the background dispatcher queue.
    print("\nShutting down gracefully...")
    await client.shutdown()
    print("Done!")


if __name__ == "__main__":
    asyncio.run(main())
