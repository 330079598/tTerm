// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { AboutDialog } from "@/components/AboutDialog"

const { openUrl } = vi.hoisted(() => ({ openUrl: vi.fn() }))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }))
vi.mock("@tauri-apps/api/app", () => ({ getVersion: () => Promise.resolve("9.8.7") }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number; version?: string }) =>
      options?.version ?? (options?.count !== undefined ? `${key}:${options.count}` : key),
  }),
}))

const licenses = {
  packages: [
    {
      name: "russh",
      version: "0.63.3",
      license: "Apache-2.0",
      source: "rust",
      url: "https://github.com/Eugeny/russh",
      files: [{ name: "Apache-2.0", spdx: true, text: 0 }],
    },
    {
      name: "@xterm/xterm",
      version: "6.0.0",
      license: "MIT",
      source: "npm",
      url: "https://xtermjs.org",
      files: [{ name: "LICENSE", text: 1 }],
    },
  ],
  texts: ["Apache License text", "Copyright (c) The xterm.js authors"],
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  openUrl.mockClear()
})

describe("AboutDialog", () => {
  it("shows the version and opens acknowledged projects", async () => {
    render(<AboutDialog open onOpenChange={() => {}} />)

    expect(await screen.findByText("9.8.7")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: /Tauri/ }))
    expect(openUrl).toHaveBeenCalledWith("https://tauri.app")
  })

  it("lists, filters and expands the bundled licenses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(new Response(JSON.stringify(licenses))))
    )
    render(<AboutDialog open onOpenChange={() => {}} />)

    fireEvent.click(screen.getByRole("button", { name: /about\.licenses/ }))
    expect(await screen.findByText("about.licensesDesc:2")).toBeTruthy()
    expect(screen.getByText("@xterm/xterm")).toBeTruthy()

    fireEvent.change(screen.getByPlaceholderText("about.licensesSearch"), {
      target: { value: "apache" },
    })
    expect(screen.queryByText("@xterm/xterm")).toBeNull()

    fireEvent.click(screen.getByRole("button", { name: /russh/ }))
    expect(screen.getByText("Apache License text")).toBeTruthy()
    expect(screen.getByText("Apache-2.0 · about.standardText")).toBeTruthy()

    fireEvent.click(screen.getByRole("button", { name: /about\.back/ }))
    expect(screen.getByText("about.acknowledgements")).toBeTruthy()
  })
})
