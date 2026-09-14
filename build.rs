fn main() {
    let mut build = cxx_build::bridge("src/nam_ffi/mod.rs");

    build
        .std("c++20")
        .define("NAM_ENABLE_A2_FAST", None)
        .include(".")
        .include("nam_core/NAM")
        .include("nam_core/Dependencies/eigen")
        .include("nam_core/Dependencies/nlohmann")
        .file("src/nam_ffi/nam_shim.cc");

    // Add ALL the NAM source files to the build.
    for entry in glob::glob("nam_core/NAM/**/*.cpp").expect("bad glob") {
        let path = entry.expect("glob entry");
        build.file(&path);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    build.compile("nam_ffi");

    // Force load ALL symbols - prevent linker from stripping out unused symbols
    println!("cargo:rustc-link-arg=-Wl,-all_load");
}
