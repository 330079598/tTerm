// Replaces `@tauri-apps/plugin-os` when Vite runs with `--mode parity`.
//
// The reference browser is WebKit on macOS, but `navigator` cannot tell the
// frontend that: `platform()` has to answer like the native app so keyboard
// shortcuts and window chrome take their macOS branches. The values come from
// `parity/screens.json`'s `platform` block (fixed to macOS by default) instead
// of being hard-coded here, so a future Windows reference only edits the
// manifest.

import manifest from "@manifest"

export type Platform =
  | "linux"
  | "macos"
  | "ios"
  | "freebsd"
  | "dragonfly"
  | "netbsd"
  | "openbsd"
  | "solaris"
  | "android"
  | "windows"
export type OsType = "linux" | "windows" | "macos" | "ios" | "android"
export type Family = "unix" | "windows"
export type Arch =
  | "x86"
  | "x86_64"
  | "arm"
  | "aarch64"
  | "mips"
  | "mips64"
  | "powerpc"
  | "powerpc64"
  | "riscv64"
  | "s390x"
  | "sparc64"

const info = manifest.platform

/** The operating system, e.g. `macos`. */
export function platform(): Platform {
  return info.os as Platform
}

/** The operating system version. */
export function version(): string {
  return info.version
}

/** The operating system family. */
export function family(): Family {
  return info.family as Family
}

/** The operating system type. */
export function type(): OsType {
  return info.type as OsType
}

/** The CPU architecture. */
export function arch(): Arch {
  return info.arch as Arch
}

/** The executable extension, empty on macOS. */
export function exeExtension(): string {
  return info.exeExtension
}

/** The line ending used by the platform. */
export function eol(): string {
  return info.eol
}
