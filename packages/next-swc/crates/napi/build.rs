extern crate napi_build;

fn main() {
    // Generates, stores build-time information as static values.
    // There are some places relying on correct values for this (i.e telemetry),
    // So failing build if this fails.
    shadow_rs::new().expect("Should able to generate build time information");

    napi_build::setup();

    #[cfg(not(target_arch = "wasm32"))]
    turbopack_binding::turbo::tasks_build::generate_register();
}
