fn main() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/xscreen.ico")
        .set_manifest_file("assets/app.manifest");
    if let Err(error) = res.compile() {
        panic!("Could not embed the Windows resource: {error}");
    }
    println!("cargo:rerun-if-changed=assets/xscreen.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
}
