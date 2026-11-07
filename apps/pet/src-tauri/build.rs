fn main() {
    let target = std::env::var("TARGET").expect("Cargo target triple");
    if target.ends_with("-windows-msvc") {
        // Tauri embeds its manifest only in application binaries. Unit-test binaries
        // also need Common Controls v6 to load TaskDialogIndirect from comctl32.dll.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        tauri_build::try_build(attributes).expect("Tauri build setup failed");
    } else {
        tauri_build::build();
    }
}
