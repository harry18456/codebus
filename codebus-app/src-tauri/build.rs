fn main() {
    // tauri-build only registers rerun-if-changed on tauri.conf.json and
    // capabilities/; the icons/ directory is invisible to cargo, so a fresh
    // `cargo tauri icon` run leaves the .exe holding the old Windows
    // resource. Explicitly watch the icon assets that get baked into the
    // binary so a `.svg` / `.ico` / `.png` edit triggers a rebuild.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=icons/source.svg");
    tauri_build::build()
}
