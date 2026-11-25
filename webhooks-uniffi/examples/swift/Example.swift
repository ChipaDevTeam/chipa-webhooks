import Foundation
import ChipaWebhooks // The module name depends on how the framework is integrated

/*
 * 1. Define a custom Matcher (Rule)
 * This intercepts `dispatch` calls and decides which template to use based on the payload.
 * If it returns nil, the dispatcher falls back to the next matcher or "default".
 */
class AlertMatcher: MatcherCallback {
    func matchTemplate(payloadJson: String) -> String? {
        // Simple substring check for demonstration.
        // In a real application, you would decode the JSON using JSONDecoder or JSONSerialization.
        if payloadJson.contains("\"level\":\"critical\"") || payloadJson.contains("\"level\": \"critical\"") {
            return "alerts"
        }
        return nil
    }
}

@main
struct WebhookExample {
    static func main() async {
        print("--- Chipa Webhooks Swift Example ---\n")

        // 2. Configuration with Multiple Destinations and Templates
        // The client takes a JSON string config.
        let configJson = """
        {
          "destinations": [
            {
              "name": "my-discord",
              "credentials": {
                "type": "discord",
                "webhook_url": "https://discord.com/api/webhooks/0000/fake-token"
              }
            },
            {
              "name": "my-telegram",
              "credentials": {
                "type": "telegram",
                "token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11",
                "chat_id": -100123456789
              }
            }
          ],
          "templates": {
            "default": "New event: {{message}}",
            "alerts": "🚨 [ALERT] 🚨 {{message}}"
          }
        }
        """

        do {
            // Initialize the client
            let client = try WebhookClient(configJson: configJson)

            // 3. Register the custom rule
            client.registerMatcher(matcher: AlertMatcher())

            // 4. Sending Sync Messages
            // The send/dispatch methods take raw JSON strings which the Rust core parses internally.
            print("[Sync] Sending direct message using 'default' template...")
            try client.sendSync(templateName: "default", payloadJson: "{\"message\": \"Hello from Swift Sync Send!\"}")

            print("[Sync] Dispatching a critical alert...")
            // This will be routed to the 'alerts' template by our AlertMatcher.
            try client.dispatchSync(payloadJson: "{\"level\": \"critical\", \"message\": \"The database is down!\"}")

            print("[Sync] Dispatching a normal info event...")
            // This won't match our AlertMatcher, so it falls back to the 'default' template.
            try client.dispatchSync(payloadJson: "{\"level\": \"info\", \"message\": \"Routine backup completed.\"}")

            // 5. Runtime Template Mutations
            print("\n[Mutations] Registering a new runtime template...")
            try client.registerTemplate(name: "runtime_test", template: "This is a runtime template: {{message}}")
            try client.sendSync(templateName: "runtime_test", payloadJson: "{\"message\": \"Hello Runtime from Swift!\"}")

            print("[Mutations] Updating an existing template...")
            try client.updateTemplate(name: "runtime_test", template: "UPDATED runtime template: {{message}}")
            try client.sendSync(templateName: "runtime_test", payloadJson: "{\"message\": \"Hello again!\"}")

            print("[Mutations] Removing the template...")
            client.removeTemplate(name: "runtime_test")

            // 6. Sending Async Messages
            // UniFFI maps Rust async methods to Swift async/await naturally.
            print("\n[Async] Sending direct message using 'alerts' template...")
            try await client.send(templateName: "alerts", payloadJson: "{\"message\": \"Async system warning!\"}")

            print("[Async] Dispatching a critical alert...")
            try await client.dispatch(payloadJson: "{\"level\": \"critical\", \"message\": \"CPU usage at 99%!\"}")

            // 7. Graceful Shutdown
            // This flushes any pending HTTP requests in the background dispatcher queue.
            print("\nShutting down gracefully...")
            await client.shutdown()

            print("Done!")

        } catch {
            print("An error occurred: \(error)")
        }
    }
}
