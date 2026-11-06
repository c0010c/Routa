fn main() {
    // tauri CLI 的 beforeBuildCommand 会重新构建组件；直接 cargo build 也必须拒绝旧组件。
    for path in ["src", "icons", "capabilities", "nsis", "procsvc", "procsvc-proto", "Cargo.toml", "Cargo.lock", "binaries", "../src", "../public", "../dist", "../scripts", "../package.json", "../pnpm-lock.yaml", "../vite.config.ts", "../tsconfig.json", "../tsconfig.node.json", "../index.html"] {
        println!("cargo:rerun-if-changed={path}");
    }
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        // Tauri CLI 也可能直接启用 tauri/custom-protocol，应查询依赖实际模式。
        assert!(!tauri_build::is_dev(), "生产构建必须启用 custom-protocol；直接 cargo build 时加 --features custom-protocol");
        let output = std::process::Command::new("node")
            .arg("../scripts/verify-build.mjs")
            .output().expect("需要 Node.js 来校验分流服务和前端构建产物");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    // Layered child windows (the taskbar status bar) need a declared Windows 8+ target.
    const MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
    </application>
  </compatibility>
</assembly>"#;
    tauri_build::try_build(
        tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(MANIFEST)),
    )
    .expect("failed to run tauri-build");
}
