//! Windows: Icon und Versionsinfos in JumpNDodge.exe einbetten (Explorer, Taskleiste, "Apps & Features").
fn main() {
    println!("cargo:rerun-if-changed=../icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("../icon.ico");
        r.set("ProductName", "JumpNDodge");
        r.set("FileDescription", "JumpNDodge");
        r.set("CompanyName", "W Software");
        r.set("LegalCopyright", "Apache-2.0");
        if let Err(e) = r.compile() { println!("cargo:warning=Icon konnte nicht eingebettet werden: {e}"); }
    }
}
