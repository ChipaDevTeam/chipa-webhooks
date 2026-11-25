fn main() {
    #[cfg(feature = "bindgen")]
    uniffi::uniffi_bindgen_main();
    #[cfg(not(feature = "bindgen"))]
    eprintln!("compile with --features bindgen to use the uniffi-bindgen tool");
}
