fn main() {
    let embeds_manifest = embed_windows_manifest();

    let mut attributes = tauri_build::Attributes::new();
    if embeds_manifest {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    }
    if let Err(error) = tauri_build::try_build(attributes) {
        println!("{error:#}");
        std::process::exit(1);
    }
}

/// Embeds the Common Controls v6 manifest through the linker instead of the
/// resource file tauri-build links into the app alone, so the lib's test
/// binary gets it too. tTerm links `TaskDialogIndirect` (through the dialog
/// plugin), which only v6 exports; without the manifest the tests fail to
/// start with STATUS_ENTRYPOINT_NOT_FOUND. Same manifest as tauri-build's
/// default, which is left out so the app does not end up with two.
fn embed_windows_manifest() -> bool {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return false;
    }

    let manifest = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("tTerm.manifest");
    std::fs::write(
        &manifest,
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    )
    .expect("failed to write the Windows manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    true
}
