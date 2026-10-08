fn main() {
    // Bundle the gettext `.po` translations under `lang/` so the UI's `@tr(...)`
    // strings can switch language at runtime via slint::select_bundled_translation.
    // Source language is English (the msgids); `lang/zh/LC_MESSAGES/mterm.po`
    // provides the Chinese translations. No per-component context, so msgids are
    // the raw English strings.
    println!("cargo:rerun-if-changed=lang");
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new()
            .with_style("fluent".into())
            .with_bundled_translations("lang")
            .with_default_translation_context(slint_build::DefaultTranslationContext::None),
    )
    .expect("Slint build failed");

    // Embed the application icon into the Windows executable so it shows up in
    // Explorer, the taskbar and shortcuts. No-op on non-Windows targets.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/mterm.ico");
        println!("cargo:rerun-if-changed=assets/MTerm.exe.manifest");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/mterm.ico");
        // winresource 默认用包名填 FileDescription/ProductName（那会是小写的
        // "mterm"，任务管理器/资源管理器显示的应用名就来自这里），显式覆盖为
        // 产品名 MTerm。
        res.set("FileDescription", "MTerm");
        res.set("ProductName", "MTerm");
        res.set("OriginalFilename", "MTerm.exe");
        // Embed an application manifest declaring Per-Monitor DPI Awareness V2.
        // Without it the DPI-awareness level depends on winit's runtime
        // SetProcessDpiAwarenessContext call, which races: if anything touches a
        // DPI API first the call silently fails and the window jumps in size /
        // cursor offset when dragged across monitors with different scaling (#194).
        // The manifest is authoritative and applied before any code runs.
        res.set_manifest_file("assets/MTerm.exe.manifest");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed Windows icon: {e}");
        }
    }
}
