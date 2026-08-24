use prost_build::Config;

fn main() {
    Config::new()
        .out_dir(std::env::var("OUT_DIR").unwrap())
        .compile_protos(&["proto/asset.proto"], &["proto/"])
        .unwrap();
}
