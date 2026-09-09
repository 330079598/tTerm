// @vitest-environment jsdom

import { cleanup, fireEvent, render } from "@testing-library/react"
import { useEffect } from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { KeymapProvider, useKeymap } from "@/contexts/KeymapContext"

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "windows" }))
vi.mock("@/contexts/ConfigContext", () => ({
  useConfig: () => ({ config: { keymap: { bindings: {} } } }),
}))

afterEach(cleanup)

function ShortcutProbe({ onSwitchTab }: { onSwitchTab: () => void }) {
  const { registerHandler } = useKeymap()

  useEffect(() => registerHandler("tabs.switchToNth", onSwitchTab), [onSwitchTab, registerHandler])
  return null
}

describe("KeymapProvider", () => {
  it("suppresses shortcuts while a portal menu is open", () => {
    const onSwitchTab = vi.fn()
    render(
      <KeymapProvider>
        <ShortcutProbe onSwitchTab={onSwitchTab} />
      </KeymapProvider>
    )
    const menu = document.createElement("div")
    menu.setAttribute("role", "menu")
    document.body.appendChild(menu)

    fireEvent.keyDown(window, { key: "1", code: "Digit1", altKey: true })
    expect(onSwitchTab).not.toHaveBeenCalled()

    menu.remove()
    fireEvent.keyDown(window, { key: "1", code: "Digit1", altKey: true })
    expect(onSwitchTab).toHaveBeenCalledOnce()
  })

  it("suppresses shortcuts while a native select has focus", () => {
    const onSwitchTab = vi.fn()
    render(
      <KeymapProvider>
        <select aria-label="Theme">
          <option>Default</option>
        </select>
        <ShortcutProbe onSwitchTab={onSwitchTab} />
      </KeymapProvider>
    )
    const select = document.querySelector("select")!
    select.focus()

    fireEvent.keyDown(select, { key: "1", code: "Digit1", altKey: true })
    expect(onSwitchTab).not.toHaveBeenCalled()
  })
})
