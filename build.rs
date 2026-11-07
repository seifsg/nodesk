//! Embeds the app and paused icons into nodesk.exe (Explorer + tray). 🎨
fn main() {
    embed_resource::compile("assets/nodesk.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compile assets/nodesk.rc");
}
