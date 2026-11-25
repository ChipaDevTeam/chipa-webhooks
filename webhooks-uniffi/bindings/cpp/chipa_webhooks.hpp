#pragma once

#include <algorithm>
#include <bit>
#include <chrono>
#include <cstdint>
#include <exception>
#include <functional>
#include <iostream>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <streambuf>
#include <type_traits>
#include <variant>
#include <vector>

#include "chipa_webhooks_scaffolding.hpp"

namespace chipa_webhooks {
struct WebhookClient;
struct WebhookFfiError;
struct MatcherCallback;


namespace uniffi {
    struct FfiConverterWebhookClient;
} // namespace uniffi

struct WebhookClient



{
    friend uniffi::FfiConverterWebhookClient;

    WebhookClient() = delete;

    WebhookClient(WebhookClient &&) = delete;

    WebhookClient &operator=(const WebhookClient &) = delete;
    WebhookClient &operator=(WebhookClient &&) = delete;

    ~WebhookClient();
    /**
     * Create a new `WebhookClient` from a JSON config string.
     */
    static std::shared_ptr<WebhookClient> init(const std::string &config_json);
    /**
     * Resolve the template via registered matchers (falling back to "default") and send.
     *
     * `payload_json` must be a valid JSON object string.
     */
    void dispatch_sync(const std::string &payload_json);
    /**
     * Register a matcher callback. The first matcher returning `Some` wins.
     */
    void register_matcher(const std::shared_ptr<MatcherCallback> &matcher);
    /**
     * Register a new template at runtime.
     */
    void register_template(const std::string &name, const std::string &template);
    /**
     * Remove a template at runtime.
     */
    void remove_template(const std::string &name);
    /**
     * Send a webhook using the named template and a raw JSON payload string.
     *
     * `payload_json` must be a valid JSON object string.
     */
    void send_sync(const std::string &template_name, const std::string &payload_json);
    /**
     * Flush queued jobs and shut down the dispatcher.
     */
    void shutdown_sync();
    /**
     * Overwrite an existing template at runtime.
     */
    void update_template(const std::string &name, const std::string &template);

    private:
    WebhookClient(const WebhookClient &);

    WebhookClient(void *);

    void *_uniffi_internal_clone_pointer() const;

    void *instance = nullptr;
};

namespace uniffi {
struct FfiConverterWebhookFfiError;
} // namespace uniffi

struct WebhookFfiError: std::runtime_error {
    friend uniffi::FfiConverterWebhookFfiError;

    WebhookFfiError() : std::runtime_error("") {}
    WebhookFfiError(const std::string &what_arg) : std::runtime_error(what_arg) {}

    virtual ~WebhookFfiError() = default;

    virtual void throw_underlying() {
        throw *this;
    }

protected:
    virtual int32_t get_variant_idx() const {
        return 0;
    };
};
/**
 * Contains variants of WebhookFfiError
 */
namespace webhook_ffi_error {

struct Config: WebhookFfiError {
    std::string message;

    Config() : WebhookFfiError("") {}
    Config(const std::string &what_arg) : WebhookFfiError(what_arg) {}

    void throw_underlying() override {
        throw *this;
    }

protected:
    int32_t get_variant_idx() const override {
        return 1;
    }
};

struct Template: WebhookFfiError {
    std::string message;

    Template() : WebhookFfiError("") {}
    Template(const std::string &what_arg) : WebhookFfiError(what_arg) {}

    void throw_underlying() override {
        throw *this;
    }

protected:
    int32_t get_variant_idx() const override {
        return 2;
    }
};

struct Dispatch: WebhookFfiError {
    std::string message;

    Dispatch() : WebhookFfiError("") {}
    Dispatch(const std::string &what_arg) : WebhookFfiError(what_arg) {}

    void throw_underlying() override {
        throw *this;
    }

protected:
    int32_t get_variant_idx() const override {
        return 3;
    }
};

struct Internal: WebhookFfiError {
    std::string message;

    Internal() : WebhookFfiError("") {}
    Internal(const std::string &what_arg) : WebhookFfiError(what_arg) {}

    void throw_underlying() override {
        throw *this;
    }

protected:
    int32_t get_variant_idx() const override {
        return 4;
    }
};
} // namespace webhook_ffi_error



struct MatcherCallback {
    virtual ~MatcherCallback() {}
    virtual
    std::optional<std::string> match_template(const std::string &payload_json) = 0;
};

namespace uniffi {
    struct UniffiCallbackInterfaceMatcherCallback {
        static void match_template(uint64_t uniffi_handle,RustBuffer payload_json,RustBuffer & uniffi_out_return,RustCallStatus *out_status);

        static void uniffi_free(uint64_t uniffi_handle);
        static void init();
    private:
        static inline UniffiVTableCallbackInterfaceMatcherCallback vtable = UniffiVTableCallbackInterfaceMatcherCallback {
            .match_template = reinterpret_cast<void *>(&match_template),
            .uniffi_free = reinterpret_cast<void *>(&uniffi_free)
        };
    };
}

namespace uniffi {struct RustStreamBuffer: std::basic_streambuf<char> {
    RustStreamBuffer(RustBuffer *buf) {
        char* data = reinterpret_cast<char*>(buf->data);
        this->setg(data, data, data + buf->len);
        this->setp(data, data + buf->capacity);
    }
    ~RustStreamBuffer() = default;

private:
    RustStreamBuffer() = delete;
    RustStreamBuffer(const RustStreamBuffer &) = delete;
    RustStreamBuffer(RustStreamBuffer &&) = delete;

    RustStreamBuffer &operator=(const RustStreamBuffer &) = delete;
    RustStreamBuffer &operator=(RustStreamBuffer &&) = delete;
};

struct RustStream: std::basic_iostream<char> {
    RustStream(RustBuffer *buf):
        std::basic_iostream<char>(&streambuf), streambuf(RustStreamBuffer(buf)) { }

    template <typename T, typename = std::enable_if_t<std::is_arithmetic_v<T>>>
    RustStream &operator>>(T &val) {
        read(reinterpret_cast<char *>(&val), sizeof(T));

        if (std::endian::native != std::endian::big) {
            auto bytes = reinterpret_cast<char *>(&val);

            std::reverse(bytes, bytes + sizeof(T));
        }

        return *this;
    }

    template <typename T, typename = std::enable_if_t<std::is_arithmetic_v<T>>>
    RustStream &operator<<(T val) {
        if (std::endian::native != std::endian::big) {
            auto bytes = reinterpret_cast<char *>(&val);

            std::reverse(bytes, bytes + sizeof(T));
        }

        write(reinterpret_cast<char *>(&val), sizeof(T));

        return *this;
    }
private:
    RustStreamBuffer streambuf;
};


RustBuffer rustbuffer_alloc(uint64_t);
RustBuffer rustbuffer_from_bytes(const ForeignBytes &);
void rustbuffer_free(RustBuffer);
template <typename T> struct HandleMap {
    HandleMap() = default;

    std::shared_ptr<T> at(uint64_t handle) {
        std::lock_guard<std::mutex> guard(this->mutex);

        return this->map.at(handle);
    }

    uint64_t insert(std::shared_ptr<T> impl) {
        std::lock_guard<std::mutex> guard(this->mutex);

        auto handle = this->cur_handle;

        this->map.insert({ handle, impl });
        this->cur_handle += 1;

        return handle;
    }

    void erase(uint64_t handle) {
        // We store the object here to avoid re-entrant locking
        std::shared_ptr<T> cleanup;
        {
            std::lock_guard<std::mutex> guard(this->mutex);
            auto it = this->map.find(handle);
            if (it != this->map.end()) {
                cleanup = it->second;
                this->map.erase(it);
            }
        }
    }
    private:
        HandleMap(const HandleMap<T> &) = delete;
        HandleMap(HandleMap<T> &&) = delete;

        HandleMap<T> &operator=(const HandleMap<T> &) = delete;
        HandleMap<T> &operator=(HandleMap<T> &&) = delete;

        std::mutex mutex;
        uint64_t cur_handle = 0;
        std::map<uint64_t, std::shared_ptr<T>> map;
};
struct FfiConverterString {
    static std::string lift(RustBuffer buf);
    static RustBuffer lower(const std::string &);
    static std::string read(RustStream &);
    static void write(RustStream &, const std::string &);
    static uint64_t allocation_size(const std::string &);
};


struct FfiConverterWebhookClient {
    static std::shared_ptr<WebhookClient> lift(void *);
    static void *lower(const std::shared_ptr<WebhookClient> &);
    static std::shared_ptr<WebhookClient> read(RustStream &);
    static void write(RustStream &, const std::shared_ptr<WebhookClient> &);
    static uint64_t allocation_size(const std::shared_ptr<WebhookClient> &);
private:
};

struct FfiConverterWebhookFfiError {
    static std::shared_ptr<WebhookFfiError> lift(RustBuffer buf);
    static RustBuffer lower(const WebhookFfiError &);
    static std::shared_ptr<WebhookFfiError> read(RustStream &stream);
    static void write(RustStream &stream, const WebhookFfiError &);
    static uint64_t allocation_size(const WebhookFfiError &);
};


struct FfiConverterMatcherCallback {
    static std::shared_ptr<MatcherCallback> lift(uint64_t);
    static uint64_t lower(const std::shared_ptr<MatcherCallback> &);
    static std::shared_ptr<MatcherCallback> read(RustStream &);
    static void write(RustStream &, const std::shared_ptr<MatcherCallback> &);
    static uint64_t allocation_size(const std::shared_ptr<MatcherCallback> &);

    inline static HandleMap<MatcherCallback> handle_map = {};
};
struct FfiConverterOptionalString {
    static std::optional<std::string> lift(RustBuffer buf);
    static RustBuffer lower(const std::optional<std::string>& val);
    static std::optional<std::string> read(RustStream &stream);
    static void write(RustStream &stream, const std::optional<std::string>& value);
    static uint64_t allocation_size(const std::optional<std::string> &val);
};
} // namespace uniffi

} // namespace chipa_webhooks