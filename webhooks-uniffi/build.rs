fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let c_feature_enabled = std::env::var("CARGO_FEATURE_C").is_ok();

    println!("cargo:rerun-if-changed=src/ffi_c.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=bindings/c/webhooks.h");

    if !c_feature_enabled {
        return;
    }

    let out_dir = std::path::Path::new(&crate_dir).join("bindings").join("c");
    std::fs::create_dir_all(&out_dir).expect("failed to create bindings/c dir");

    // webhooks.h is maintained manually in bindings/c/webhooks.h because
    // cbindgen 0.27 cannot expand #[cfg(feature = "c")] gates during the
    // build script (the feature flag is not visible to cbindgen's own parser
    // pass). The header is the source of truth for the C API contract.
    //
    // If cbindgen ever gains reliable cfg-expansion support, replace this
    // block with a .write_to_file() call and delete the manual header.
    let config = cbindgen::Config::from_file("cbindgen.toml").unwrap_or_default();
    if let Ok(bindings) = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        // Only overwrite the header if cbindgen produced meaningful content
        // (i.e. it found at least one extern fn beyond the boilerplate includes).
        let mut buf = Vec::new();
        bindings.write(&mut buf);
        let generated = String::from_utf8_lossy(&buf);
        if generated.contains("webhook_client_new") {
            std::fs::write(out_dir.join("webhooks.h"), buf).expect("failed to write webhooks.h");
        }
        // Otherwise leave the manually-maintained header in place.
    }
}
