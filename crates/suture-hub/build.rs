fn main() {
    tonic_build::configure()
        .type_attribute(".", "#[derive(Eq)]")
        .compile_protos(&["proto/suture.proto"], &["proto"])
        .unwrap_or_else(|e| {
            println!("cargo:warning=Proto compilation failed: {e}");
            println!("cargo:warning=gRPC will not be available");
        });
}
