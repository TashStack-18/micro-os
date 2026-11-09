fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rerun-if-changed=linker.ld");
    for asm in ["src/boot/entry.S", "src/trap/trap.S", "src/task/switch.S"] {
        println!("cargo:rerun-if-changed={asm}");
    }
}
