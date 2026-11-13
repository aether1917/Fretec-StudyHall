fn main() {
    // 不使用 tauri-build 默认嵌入的 app manifest：默认清单只链接进 bin 目标，
    // 单元测试（lib test）链接 tauri 时会缺少 Common Controls v6 清单，
    // 启动即 STATUS_ENTRYPOINT_NOT_FOUND。改用下面的 rustc-link-arg 统一补清单。
    let attributes = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new_without_app_manifest(),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");

    // 为所有链接目标（含单元测试）嵌入 Common Controls v6 清单。
    #[cfg(windows)]
    {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
