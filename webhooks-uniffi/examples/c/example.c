#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Include the generated or manually maintained C header */
#include "webhooks.h"

/* 
 * 1. Define a custom Matcher (Rule)
 * This function will be called during `webhook_client_dispatch`.
 * It receives the raw JSON string payload.
 * If it returns a string (the template name), that template will be used.
 * If it returns NULL, the dispatcher falls back to the next matcher or "default".
 */
const char* my_alert_matcher(const char* payload_json) {
    /* Simple substring check for demonstration. 
       In a real application, you might use a C JSON parser like cJSON. */
    if (strstr(payload_json, "\"level\":\"critical\"") != NULL) {
        return "alerts";
    }
    return NULL;
}

int main(void) {
    char err_buf[512] = {0};
    
    printf("--- Chipa Webhooks C Example ---\n\n");

    /* 
     * 2. Configuration with Multiple Destinations and Templates
     * The Rust core expects this to be valid JSON. 
     */
    const char* config_json = 
        "{"
        "  \"destinations\": ["
        "    {"
        "      \"name\": \"my-discord\","
        "      \"credentials\": {"
        "        \"type\": \"discord\","
        "        \"webhook_url\": \"https://discord.com/api/webhooks/0000/fake-token\""
        "      }"
        "    },"
        "    {"
        "      \"name\": \"my-telegram\","
        "      \"credentials\": {"
        "        \"type\": \"telegram\","
        "        \"token\": \"123456:ABC-DEF1234ghIkl\","
        "        \"chat_id\": -100123456789"
        "      }"
        "    }"
        "  ],"
        "  \"templates\": {"
        "    \"default\": \"New event: {{message}}\","
        "    \"alerts\": \"🚨 [ALERT] 🚨 {{message}}\""
        "  }"
        "}";

    /* Initialize the client */
    printf("Initializing webhook client...\n");
    WebhookClientHandle* client = webhook_client_new(config_json, err_buf, sizeof(err_buf));
    if (client == NULL) {
        fprintf(stderr, "Failed to create client: %s\n", err_buf);
        return 1;
    }

    /* 3. Register the custom rule */
    printf("Registering alert matcher...\n");
    webhook_client_register_matcher(client, my_alert_matcher);

    /* 4. Sending Messages 
     * The `send` and `dispatch` functions take raw JSON strings.
     * The Rust core parses the JSON payload into a serde_json::Value to render Handlebars templates.
     */
    
    printf("\n[Send] Sending direct message using 'default' template...\n");
    const char* payload1 = "{\"message\": \"Hello from C Send!\"}";
    if (webhook_client_send(client, "default", payload1, err_buf, sizeof(err_buf)) != 0) {
        fprintf(stderr, "Send error: %s\n", err_buf);
    }

    printf("[Dispatch] Dispatching a critical alert...\n");
    /* This will match our my_alert_matcher and use the 'alerts' template */
    const char* payload2 = "{\"level\":\"critical\", \"message\":\"The database is down!\"}";
    if (webhook_client_dispatch(client, payload2, err_buf, sizeof(err_buf)) != 0) {
        fprintf(stderr, "Dispatch error: %s\n", err_buf);
    }

    printf("[Dispatch] Dispatching a normal info event...\n");
    /* This won't match, so it falls back to 'default' */
    const char* payload3 = "{\"level\":\"info\", \"message\":\"Routine backup completed.\"}";
    if (webhook_client_dispatch(client, payload3, err_buf, sizeof(err_buf)) != 0) {
        fprintf(stderr, "Dispatch error: %s\n", err_buf);
    }

    /* 5. Graceful Shutdown
     * Freeing the client will automatically flush any background async tasks
     * ensuring that HTTP requests complete before the program exits.
     */
    printf("\nShutting down gracefully...\n");
    webhook_client_free(client);
    printf("Done!\n");

    return 0;
}