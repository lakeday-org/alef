fn main() {
    swift_bridge_build::parse_bridges(["src/lib.rs"]).write_all_concatenated("swift-generated", "Fixture");
    println!("cargo:rerun-if-changed=src/lib.rs");
}
