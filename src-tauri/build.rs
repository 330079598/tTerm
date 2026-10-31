fn main() {
    if let Err(error) = prepare_conpty() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    // tauri-build compiles icons/icon.ico into the Windows resource the
    // taskbar and window icons come from, and generate_context! embeds the
    // icons for the other platforms, but neither tells Cargo to watch them,
    // so after `pnpm icons:update` the app kept its old icon until a clean
    // build.
    println!("cargo:rerun-if-changed=icons");
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

/// Stages the pinned Windows runtime so theme queries reach the frontend on older Windows.
fn prepare_conpty() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../scripts/prepare-conpty.ps1");
    if std::env::var("CARGO_CFG_TARGET_OS")? != "windows" {
        return Ok(());
    }

    let out_dir = std::env::var("OUT_DIR")?;
    let profile_dir = std::path::Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .ok_or("Cannot locate Cargo's target directory for ConPTY")?;
    // Unit test executables live in deps; portable-pty loads conpty.dll beside the executable.
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            "../scripts/prepare-conpty.ps1",
            "-Architecture",
            &std::env::var("CARGO_CFG_TARGET_ARCH")?,
            "-TestDirectory",
        ])
        .arg(profile_dir.join("deps"))
        .status()?;
    if !status.success() {
        return Err(
            "Failed to prepare ConPTY. Check the download/cache error above and rebuild.".into(),
        );
    }
    Ok(())
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
