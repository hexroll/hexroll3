#[cfg(target_os = "windows")]
extern crate embed_resource;

fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.contains("windows") {
        #[cfg(target_os = "windows")]
        embed_resource::compile("icon.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
