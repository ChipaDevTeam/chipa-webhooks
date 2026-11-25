#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifndef UNIFFI_CPP_INTERNALSTRUCTS
#define UNIFFI_CPP_INTERNALSTRUCTS
struct ForeignBytes {
    int32_t len;
    uint8_t *data;
};

struct RustBuffer {
    uint64_t capacity;
    uint64_t len;
    uint8_t *data;
};

struct RustCallStatus {
    int8_t code;
    RustBuffer error_buf;
};

#endif
struct UniffiVTableCallbackInterfaceMatcherCallback {
    void * match_template;
    void * uniffi_free;
};
void * uniffi_chipa_webhooks_fn_clone_webhookclient(void * ptr, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_free_webhookclient(void * ptr, RustCallStatus *out_status);
void * uniffi_chipa_webhooks_fn_constructor_webhookclient_new(RustBuffer config_json, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_method_webhookclient_dispatch_sync(void * ptr, RustBuffer payload_json, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_method_webhookclient_register_matcher(void * ptr, uint64_t matcher, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_method_webhookclient_register_template(void * ptr, RustBuffer name, RustBuffer template, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_method_webhookclient_remove_template(void * ptr, RustBuffer name, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_method_webhookclient_send_sync(void * ptr, RustBuffer template_name, RustBuffer payload_json, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_method_webhookclient_shutdown_sync(void * ptr, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_method_webhookclient_update_template(void * ptr, RustBuffer name, RustBuffer template, RustCallStatus *out_status);
void uniffi_chipa_webhooks_fn_init_callback_vtable_matchercallback(const UniffiVTableCallbackInterfaceMatcherCallback & vtable);
RustBuffer ffi_chipa_webhooks_rustbuffer_alloc(uint64_t size, RustCallStatus *out_status);
RustBuffer ffi_chipa_webhooks_rustbuffer_from_bytes(ForeignBytes bytes, RustCallStatus *out_status);
void ffi_chipa_webhooks_rustbuffer_free(RustBuffer buf, RustCallStatus *out_status);
RustBuffer ffi_chipa_webhooks_rustbuffer_reserve(RustBuffer buf, uint64_t additional, RustCallStatus *out_status);
uint16_t uniffi_chipa_webhooks_checksum_method_webhookclient_dispatch_sync();
uint16_t uniffi_chipa_webhooks_checksum_method_webhookclient_register_matcher();
uint16_t uniffi_chipa_webhooks_checksum_method_webhookclient_register_template();
uint16_t uniffi_chipa_webhooks_checksum_method_webhookclient_remove_template();
uint16_t uniffi_chipa_webhooks_checksum_method_webhookclient_send_sync();
uint16_t uniffi_chipa_webhooks_checksum_method_webhookclient_shutdown_sync();
uint16_t uniffi_chipa_webhooks_checksum_method_webhookclient_update_template();
uint16_t uniffi_chipa_webhooks_checksum_constructor_webhookclient_new();
uint16_t uniffi_chipa_webhooks_checksum_method_matchercallback_match_template();
uint32_t ffi_chipa_webhooks_uniffi_contract_version();
#ifdef __cplusplus
}
#endif