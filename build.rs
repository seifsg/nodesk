//! Embeds the icons (Explorer + tray) and the app manifest (themes, per-monitor DPI) into nodesk.exe. 🎨
fn main() {
    println!("cargo:rerun-if-changed=assets");
    embed_resource::compile("assets/nodesk.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compile assets/nodesk.rc");
}
