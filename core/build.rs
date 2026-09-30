use prost_build::Config;

fn main() {
    Config::new()
        .out_dir(std::env::var("OUT_DIR").unwrap())
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&["proto/asset.proto"], &["proto/"])
        .unwrap();
}
