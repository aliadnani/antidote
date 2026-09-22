fn main() {
    let mut build = cxx_build::bridge("src/nam_ffi/mod.rs");

    // TODO: There's probably a better way to do this with cmake-rs
    build
        .std("c++20")
        .define("NAM_ENABLE_A2_FAST", None)
        .define("NAM_SAMPLE_FLOAT", None)
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

    println!("cargo:rerun-if-changed=src/nam_ffi/nam_shim.cc");
    println!("cargo:rerun-if-changed=src/nam_ffi/nam_shim.h");

    // Force the linker to pull in every object in the static lib so the
    // config-parser static initializers in otherwise-unreferenced NAM
    // translation units (e.g. container.cpp) actually run. rustc's
    // `+whole-archive` modifier is position-independent, unlike a raw
    // -Wl,--whole-archive link-arg, which only affects archives listed after it.
    build.link_lib_modifier("+whole-archive");
    build.compile("nam_ffi");
}
