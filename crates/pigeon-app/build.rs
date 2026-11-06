//! Puts the icon and product details into the Windows program file.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/pigeon.ico");
    // Asked this way so it also works when building for Windows from another system.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/icon/pigeon.ico");
    resource.set("ProductName", "Pigeon");
    resource.set("FileDescription", "Pigeon");
    resource.compile().expect("the Windows icon could not be added");
}
