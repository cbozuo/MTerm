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
        // GNU 工具链上 winresource 会按 "x86_64-w64-mingw32-windres"/"ar" 这两个
        // 名字去 %PATH% 里碰运气找工具（MSVC 走注册表找 rc.exe，不受影响）。
        // 构建机的 PATH 未必带 MinGW bin（干净环境起的 CI/cargo 就没有），找不到
        // 就静默丢图标 + DPI manifest（只留一条 warning），所以这里主动探测常见
        // 安装位置，找到就显式给出绝对路径。三个坑：
        //   * set_toolkit_path 只影响子进程工作目录，不参与工具查找，别用错；
        //   * 优先选带 triple 前缀的 windres：MSYS2 纯 binutils 安装的裸 windres
        //     预处理 .rc 要起 gcc（PATH 上往往没有），LLVM windres 自带预处理；
        //   * windres 之后还要 ar 打包 resource.o，一样会栽在 PATH 上。
        // 用户显式设置了 WINDRES/AR 时以它们为准，不在这里覆盖。
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
            let tool_dirs = [
                "C:/llvm-mingw/bin",
                "C:/msys64/mingw64/bin",
                "C:/msys64/ucrt64/bin",
                "C:/msys64/clang64/bin",
                "C:/mingw64/bin",
            ];
            let find_tool = |names: &[&str]| {
                tool_dirs.iter().find_map(|dir| {
                    names
                        .iter()
                        .map(|name| std::path::Path::new(dir).join(name))
                        .find(|path| path.is_file())
                })
            };
            if std::env::var_os("WINDRES").is_none() {
                // llvm-mingw 排最前：它的 windres 自带预处理；MSYS2 纯 binutils
                // 安装的裸 windres 要起 gcc，放后面兜底（那种目录里通常也有 gcc）。
                if let Some(windres) = find_tool(&[
                    "x86_64-w64-mingw32-windres.exe",
                    "aarch64-w64-mingw32-windres.exe",
                    "windres.exe",
                ]) {
                    res.set_windres_path(&windres.display().to_string());
                }
            }
            if std::env::var_os("AR").is_none() {
                if let Some(ar) = find_tool(&["x86_64-w64-mingw32-ar.exe", "ar.exe"]) {
                    res.set_ar_path(&ar.display().to_string());
                }
            }
        }
        // winresource 的 windres（GNU）/rc.exe（MSVC）都要往 %TMP% 写预处理临时
        // 文件。受控环境下（进程信任式写过滤：新编译的未签名构建子进程视为不可
        // 信，用户 Temp 拒写、只放行工作区），windres 会以
        //   Unable to create temp file: Permission denied
        // 静默失败，这里只剩一条 warning，exe 的图标 + DPI manifest 就丢了
        // （#icon-lost 2026-10-15，实测 Python/bash 启动同一 windres 可过、
        // Rust Command 启动必挂，与 cwd/TMP 形态无关，只看临时目录落点）。
        // 把 TMP/TEMP 指到 OUT_DIR —— cargo 管理的工作区内目录，必定可写。
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
        std::env::set_var("TMP", &out_dir);
        std::env::set_var("TEMP", &out_dir);
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed Windows icon: {e}");
        }
    }
}
