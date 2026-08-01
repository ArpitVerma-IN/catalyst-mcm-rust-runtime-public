use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to invalidate the built crate if the wrapper header changes
    println!("cargo:rerun-if-changed=../include/catalyst_bindings/capi.h");

    let bindings = bindgen::Builder::default()
        .header("../include/catalyst_bindings/capi.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings for Catalyst C-API");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
