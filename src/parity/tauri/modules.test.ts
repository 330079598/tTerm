/// <reference types="vite/client" />
import { describe, expect, it } from "vitest"
import { PARITY_TAURI_MODULES, parityTauriAliases } from "@/parity/tauri/modules"

const implementationFiles = Object.keys(import.meta.glob("./*.ts"))
const viteConfigs = import.meta.glob("../../../vite.config.ts", {
  eager: true,
  query: "?raw",
  import: "default",
})
const viteConfig = Object.values(viteConfigs)[0] as string

describe("parity Tauri modules", () => {
  it("lists every module the frontend imports once", () => {
    const specifiers = PARITY_TAURI_MODULES.map((module) => module.specifier)
    expect(new Set(specifiers).size).toBe(specifiers.length)
    expect(specifiers).toHaveLength(10)
  })

  it("builds the Vite aliases from that list", () => {
    expect(viteConfig).toContain("parityTauriAliases")
  })

  it.each(PARITY_TAURI_MODULES)("maps $specifier to $file", ({ specifier, file }) => {
    const implementation = `./${file.split("/").slice(-1)[0] ?? ""}`
    expect(implementationFiles, `${file} must exist`).toContain(implementation)
    const aliases = new Map(parityTauriAliases((path) => `/root/${path}`))
    expect(aliases.get(specifier), `${specifier} must have a parity alias`).toBe(`/root/${file}`)
  })
})
