import chipa_webhooks.*
import kotlinx.coroutines.runBlocking

/**
 * 1. Define a custom Matcher (Rule)
 * This intercepts `dispatch` calls and decides which template to use based on the payload.
 * If it returns null, the dispatcher falls back to the next matcher or "default".
 */
class AlertMatcher : MatcherCallback {
    override fun matchTemplate(payloadJson: String): String? {
        // A simple substring check. In a real app, you would parse the JSON
        // with something like kotlinx.serialization or Gson.
        if (payloadJson.contains("\"level\":\"critical\"") || payloadJson.contains("\"level\": \"critical\"")) {
            return "alerts"
        }
        return null
    }
}

fun main() {
    println("--- Chipa Webhooks Kotlin Example ---\n")

    // 2. Configuration with Multiple Destinations and Templates
    // The client takes a JSON string config.
    val configJson = """
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
    """.trimIndent()

    // Initialize the client
    val client = WebhookClient(configJson)

    // 3. Register the custom rule
    client.registerMatcher(AlertMatcher())

    // 4. Sending Sync Messages
    // The send/dispatch methods take raw JSON strings which the Rust core parses internally.
    println("[Sync] Sending direct message using 'default' template...")
    client.sendSync("default", """{"message": "Hello from Kotlin Sync Send!"}""")

    println("[Sync] Dispatching a critical alert...")
    // This will be routed to the 'alerts' template by our AlertMatcher.
    client.dispatchSync("""{"level": "critical", "message": "The database is down!"}""")

    println("[Sync] Dispatching a normal info event...")
    // This won't match our AlertMatcher, so it falls back to the 'default' template.
    client.dispatchSync("""{"level": "info", "message": "Routine backup completed."}""")

    // 5. Runtime Template Mutations
    println("\n[Mutations] Registering a new runtime template...")
    client.registerTemplate("runtime_test", "This is a runtime template: {{message}}")
    client.sendSync("runtime_test", """{"message": "Hello Runtime from Kotlin!"}""")

    println("[Mutations] Updating an existing template...")
    client.updateTemplate("runtime_test", "UPDATED runtime template: {{message}}")
    client.sendSync("runtime_test", """{"message": "Hello again!"}""")

    println("[Mutations] Removing the template...")
    client.removeTemplate("runtime_test")

    // 6. Sending Async Messages
    // UniFFI maps Rust `async` methods to Kotlin `suspend` functions.
    // We use `runBlocking` here to demonstrate them in a blocking main function.
    runBlocking {
        println("\n[Async] Sending direct message using 'alerts' template...")
        client.send("alerts", """{"message": "Async system warning!"}""")

        println("[Async] Dispatching a critical alert...")
        client.dispatch("""{"level": "critical", "message": "CPU usage at 99%!"}""")

        // 7. Graceful Shutdown
        // This flushes any pending HTTP requests in the background dispatcher queue.
        println("\nShutting down gracefully...")
        client.shutdown()
    }

    println("Done!")
}
