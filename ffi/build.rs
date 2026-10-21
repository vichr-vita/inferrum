//! Generate Rust transport types from the shared schema.

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=proto/bonus.proto");
    prost_build::Config::new()
        .boxed(".inferrum.bonus.v1.BonusResponse.outcome.result")
        .compile_protos(&["proto/bonus.proto"], &["proto"])
}
