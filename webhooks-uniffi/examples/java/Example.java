import uniffi.chipa_webhooks.*;

/**
 * 1. Define a custom Matcher (Rule)
 * This intercepts `dispatch` calls and decides which template to use based on the payload.
 * If it returns null, the dispatcher falls back to the next matcher or "default".
 */
class AlertMatcher implements MatcherCallback {
    @Override
    public String matchTemplate(String payloadJson) {
        // A simple substring check. In a real app, you would parse the JSON
        // with something like Jackson or Gson.
        if (payloadJson.contains("\"level\":\"critical\"") || payloadJson.contains("\"level\": \"critical\"")) {
            return "alerts";
        }
        return null;
    }
}

public class Example {
    public static void main(String[] args) {
        System.out.println("--- Chipa Webhooks Java Example ---\n");

        // 2. Configuration with Multiple Destinations and Templates
        // The client takes a JSON string config.
        String configJson = "{\n" +
            "  \"destinations\": [\n" +
            "    {\n" +
            "      \"name\": \"my-discord\",\n" +
            "      \"credentials\": {\n" +
            "        \"type\": \"discord\",\n" +
            "        \"webhook_url\": \"https://discord.com/api/webhooks/0000/fake-token\"\n" +
            "      }\n" +
            "    },\n" +
            "    {\n" +
            "      \"name\": \"my-telegram\",\n" +
            "      \"credentials\": {\n" +
            "        \"type\": \"telegram\",\n" +
            "        \"token\": \"123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11\",\n" +
            "        \"chat_id\": -100123456789\n" +
            "      }\n" +
            "    }\n" +
            "  ],\n" +
            "  \"templates\": {\n" +
            "    \"default\": \"New event: {{message}}\",\n" +
            "    \"alerts\": \"🚨 [ALERT] 🚨 {{message}}\"\n" +
            "  }\n" +
            "}";

        try {
            // Initialize the client
            WebhookClient client = new WebhookClient(configJson);

            // 3. Register the custom rule
            client.registerMatcher(new AlertMatcher());

            // 4. Sending Sync Messages
            // The send/dispatch methods take raw JSON strings which the Rust core parses internally.
            System.out.println("[Sync] Sending direct message using 'default' template...");
            client.sendSync("default", "{\"message\": \"Hello from Java Sync Send!\"}");

            System.out.println("[Sync] Dispatching a critical alert...");
            // This will be routed to the 'alerts' template by our AlertMatcher.
            client.dispatchSync("{\"level\": \"critical\", \"message\": \"The database is down!\"}");

            System.out.println("[Sync] Dispatching a normal info event...");
            // This won't match our AlertMatcher, so it falls back to the 'default' template.
            client.dispatchSync("{\"level\": \"info\", \"message\": \"Routine backup completed.\"}");

            // 5. Runtime Template Mutations
            System.out.println("\n[Mutations] Registering a new runtime template...");
            client.registerTemplate("runtime_test", "This is a runtime template: {{message}}");
            client.sendSync("runtime_test", "{\"message\": \"Hello Runtime from Java!\"}");

            System.out.println("[Mutations] Updating an existing template...");
            client.updateTemplate("runtime_test", "UPDATED runtime template: {{message}}");
            client.sendSync("runtime_test", "{\"message\": \"Hello again!\"}");

            System.out.println("[Mutations] Removing the template...");
            client.removeTemplate("runtime_test");

            // 6. Graceful Shutdown
            // This flushes any pending HTTP requests in the background dispatcher queue.
            System.out.println("\nShutting down gracefully...");
            client.shutdownSync();
            System.out.println("Done!");

        } catch (WebhookFfiException e) {
            System.err.println("Webhook error: " + e.getMessage());
        }
    }
}
