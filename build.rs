fn main() {
    // println!("cargo:rustc-link-search=native=vendor");
    // println!("cargo:rustc-link-lib=GetCoreTempInfo");
    // println!("cargo:rerun-if-changed=vendor/GetCoreTempInfo.lib");

    #[cfg(windows)]
    {
        if std::env::var("PROFILE").ok().as_deref() != Some("release") {
        } else if std::env::var_os("RC_PATH").is_none() && find_rc_path().is_none() {
            println!("cargo:warning=rc.exe not found, skip Windows resource");
        } else {
            winresource::WindowsResource::new().compile().unwrap();
        }
    }
    // fluent material cupertino cosmic qt native
    let conf = slint_build::CompilerConfiguration::default().with_style("fluent".to_owned());
    slint_build::compile_with_config("ui/page/app.slint", conf).unwrap();
    slint_build::print_rustc_flags().unwrap();
}

#[cfg(windows)]
fn find_rc_path() -> Option<()> {
    let base = std::path::Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut paths = std::fs::read_dir(base).ok()?.filter_map(Result::ok).map(|entry| entry.path().join("x64").join("rc.exe")).filter(|path| path.exists()).collect::<Vec<_>>();
    paths.sort();
    let path = paths.pop()?;
    unsafe { std::env::set_var("RC_PATH", path) };
    Some(())
}
