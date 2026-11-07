# P0-26 Windows experiment

This directory targets only `com.stone.tTerm.gpui-dev`, product `tTerm GPUI Dev`.
The Tauri experiment is `0.0.1`; the GPUI update is `0.0.2`.
Do not run either system integration installer argument or use real application data.

Run commands from the `D:\code\rust\tTerm-P0-26` repository root in PowerShell.
Capture system state before installing anything:

```powershell
./scripts/packaging/snapshot-system.ps1 -Phase before
pnpm install
pnpm tauri signer generate --ci --write-keys D:/code/rust/tTerm-p0-26-keys/test.key
git worktree add --detach D:/code/rust/tTerm-p0-26-scratch/tauri-build HEAD
git -C D:/code/rust/tTerm-p0-26-scratch/tauri-build apply D:/code/rust/tTerm-P0-26/scripts/packaging/experiment-updater.patch
./scripts/packaging/prepare-tauri.ps1 -Build
```

Do not rerun key generation or baseline capture over existing evidence.
`prepare-tauri.ps1` clears inherited signing variables without reading their values,
uses only the new public key, disables updater artifacts, and builds release NSIS
in `D:\code\rust\tTerm-p0-26-scratch\target-tauri`.
The generated installer and NSIS script must be reviewed before installation.
The repository's `src-tauri` files stay untouched; the patch deletes legacy data
migration and redirects all experiment update channels to loopback.

Install the packaging tool with a fixed version, outside the repository:

```powershell
$env:CARGO_TARGET_DIR = 'D:\code\rust\tTerm-build-P0-13'
cargo install --locked --version 0.11.8 --root D:/code/rust/tTerm-p0-26-scratch/tools cargo-packager
$env:CARGO_TARGET_DIR = 'D:\code\rust\tTerm-build-P0-13'
cargo build --workspace --release --bin tterm-gpui
```

The GPUI executable is `D:\code\rust\tTerm-build-P0-13\release\tterm-gpui.exe`.
Staging must rename it to the experiment's main binary name and include
`conpty.dll`, `OpenConsole.exe`, and `conpty-LICENSE.txt` beside it.
Installation identity, manufacturer, current-user scope, registry keys, shortcuts,
and the installation directory must match the reviewed Tauri installer.

The reproducible GPUI packaging and signing entry points are:

```powershell
./scripts/packaging/package-gpui.ps1
./scripts/packaging/sign-update.ps1
```

Tauri release installer:
`D:\code\rust\tTerm-p0-26-scratch\target-tauri\release\bundle\nsis\tTerm GPUI Dev_0.0.1_x64-setup.exe`.
GPUI release installer:
`D:\code\rust\tTerm-p0-26-scratch\packages\tTerm GPUI Dev_0.0.2_x64-setup.exe`.
The actual Tauri installation is
`C:\Users\stone\AppData\Local\Programs\tTerm GPUI Dev`.
Both generated NSIS scripts identify publisher `stone`, current-user scope,
main binary `tTerm GPUI Dev.exe`, uninstall key `tTerm GPUI Dev`, and install
location key `HKCU\Software\stone\tTerm GPUI Dev`.
The packager configuration explicitly supplies `name` because 0.11.8 otherwise
tries to change directory to its configuration file while inferring the name.

Run the service in a hidden background process, recording its PID and logs:

```powershell
Start-Process -FilePath (Get-Command node).Source -ArgumentList D:/code/rust/tTerm-P0-26/scripts/packaging/serve-update.mjs -WindowStyle Hidden -RedirectStandardOutput D:/code/rust/tTerm-p0-26-scratch/evidence/server-stdout.log -RedirectStandardError D:/code/rust/tTerm-p0-26-scratch/evidence/server-stderr.log -PassThru
```

The service has an explicit two-file allowlist. Its document root contains no
keys. Signatures use the newly generated test key only. Production signing
variables must be cleared before any signing or packaging invocation.

Full local evidence belongs in `D:\code\rust\tTerm-p0-26-scratch\evidence`.
Private keys belong only in `D:\code\rust\tTerm-p0-26-keys`.
Neither path is published or committed. The local HTTP service must bind only
`127.0.0.1:18226` and serve only signed experiment installers and manifests.
Stop before clicking **Check for updates**: those application clicks belong to
the decision maker. See the task report for the current execution state.

After the experiment, uninstall only the verified experiment installation,
capture `after` with the same snapshot script, and compare all five snapshot
files. Remove the temporary patched worktree with `git worktree remove` after
preserving its build and audit evidence; then use `git worktree prune`.
Never remove the shared build directory or either real application's data.
