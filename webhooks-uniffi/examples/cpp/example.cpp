#include <iostream>
#include <string>
#include <memory>
#include <optional>
#include <exception>

// Include the generated UniFFI C++ header.
// Make sure you've generated it using a community C++ uniffi bindgen and it's in your include path.
#include "chipa_webhooks.hpp"

using namespace chipa_webhooks;

/**
 * 1. Define a custom Matcher (Rule)
 * This intercepts `dispatch` calls and decides which template to use based on the payload.
 * If it returns std::nullopt, the dispatcher falls back to the next matcher or "default".
 */
class AlertMatcher : public MatcherCallback {
public:
    std::optional<std::string> matchTemplate(const std::string& payload_json) override {
        // Simple substring check for demonstration.
        // In a real application, you would use a JSON library like nlohmann/json.
        if (payload_json.find("\"level\":\"critical\"") != std::string::npos || 
            payload_json.find("\"level\": \"critical\"") != std::string::npos) {
            return "alerts";
        }
        return std::null;
    }
};

int main() {
    std::cout << "--- Chipa Webhooks C++ Example ---\n\n";

    // 2. Configuration with Multiple Destinations and Templates
    // The client takes a JSON string config.
    std::string config_json = R"({
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
    })";

    try {
        // Initialize the client
        // UniFFI typically uses std::make_shared for returned custom objects.
        auto client = std::make_shared<WebhookClient>(config_json);

        // 3. Register the custom rule
        client->registerMatcher(std::make_shared<AlertMatcher>());

        // 4. Sending Sync Messages
        // The send/dispatch methods take raw JSON strings which the Rust core parses internally.
        std::cout << "[Sync] Sending direct message using 'default' template...\n";
        client->sendSync("default", R"({"message": "Hello from C++ Sync Send!"})");

        std::cout << "[Sync] Dispatching a critical alert...\n";
        // This will be routed to the 'alerts' template by our AlertMatcher.
        client->dispatchSync(R"({"level": "critical", "message": "The database is down!"})");

        std::cout << "[Sync] Dispatching a normal info event...\n";
        // This won't match our AlertMatcher, so it falls back to the 'default' template.
        client->dispatchSync(R"({"level": "info", "message": "Routine backup completed."})");

        // 5. Runtime Template Mutations
        std::cout << "\n[Mutations] Registering a new runtime template...\n";
        client->registerTemplate("runtime_test", "This is a runtime template: {{message}}");
        client->sendSync("runtime_test", R"({"message": "Hello Runtime from C++!"})");

        std::cout << "[Mutations] Updating an existing template...\n";
        client->updateTemplate("runtime_test", "UPDATED runtime template: {{message}}");
        client->sendSync("runtime_test", R"({"message": "Hello again!"})");

        std::cout << "[Mutations] Removing the template...\n";
        client->removeTemplate("runtime_test");

        // 6. Graceful Shutdown
        // This flushes any pending HTTP requests in the background dispatcher queue.
        std::cout << "\nShutting down gracefully...\n";
        client->shutdownSync();

        std::cout << "Done!\n";

    } catch (const std::exception& e) {
        std::cerr << "An error occurred: " << e.what() << "\n";
        return 1;
    }

    return 0;
}