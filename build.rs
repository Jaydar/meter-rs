fn main() {
    // println!("cargo:rustc-link-search=native=vendor");
    // println!("cargo:rustc-link-lib=GetCoreTempInfo");
    // println!("cargo:rerun-if-changed=vendor/GetCoreTempInfo.lib");

    #[cfg(windows)]
    {
        if std::env::var_os("RC_PATH").is_none() {
            unsafe {
                std::env::set_var(
                    "RC_PATH",
                    r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe",
                )
            };
        }
        winresource::WindowsResource::new().compile().unwrap();
    }

    let conf = slint_build::CompilerConfiguration::default().with_style("cosmic".to_owned());
    slint_build::compile_with_config("ui/page/app.slint", conf).unwrap();
    slint_build::print_rustc_flags().unwrap();
}
