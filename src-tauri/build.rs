fn main() {
    println!("cargo:rerun-if-changed=bin");

    tauri_build::build()
}
