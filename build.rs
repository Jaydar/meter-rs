fn main() {
    // println!("cargo:rustc-link-search=native=vendor");
    // println!("cargo:rustc-link-lib=GetCoreTempInfo");
    // println!("cargo:rerun-if-changed=vendor/GetCoreTempInfo.lib");
    
    let conf = slint_build::CompilerConfiguration::default().with_style("cosmic".to_owned());
    slint_build::compile_with_config("ui/page/app.slint",conf).unwrap();
    // slint_build::compile("ui/main.slint").unwrap();
    slint_build::print_rustc_flags().unwrap();
}
