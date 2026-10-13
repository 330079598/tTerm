// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { useEffect } from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { ContextMenu } from "@/components/ContextMenu"
import { KeymapProvider, useKeymap } from "@/contexts/KeymapContext"
import type { KeymapConfig } from "@/lib/keymap/keymap"

const keymap = vi.hoisted(() => ({ current: { bindings: {} } as KeymapConfig }))

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "windows" }))
vi.mock("@/contexts/ConfigContext", () => ({
  useConfig: () => ({ config: { keymap: keymap.current } }),
}))

afterEach(() => {
  cleanup()
  keymap.current = { bindings: {} }
})

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

describe("shortcut labels", () => {
  function renderMenu() {
    return render(
      <KeymapProvider>
        <ContextMenu
          x={10}
          y={10}
          actions={[
            { label: "Rename tab", action: "rename", shortcut: "tabs.renameActive" },
            { label: "Reset terminal", action: "reset", shortcut: "terminal.reset" },
          ]}
          onAction={vi.fn()}
          onClose={vi.fn()}
        />
      </KeymapProvider>
    )
  }

  it("shows each menu item's chord, and none for an unbound action", () => {
    renderMenu()
    expect(screen.getByRole("menuitem", { name: "Rename tab" }).textContent).toBe(
      "Rename tabCtrl+Shift+R"
    )
    expect(screen.getByRole("menuitem", { name: "Reset terminal" }).textContent).toBe(
      "Reset terminal"
    )
  })

  it("shows the chord the user rebound the action to", () => {
    keymap.current = { bindings: { "tabs.renameActive": ["mod+e"], "terminal.reset": ["alt+r"] } }
    renderMenu()
    expect(screen.getByRole("menuitem", { name: "Rename tab" }).textContent).toBe(
      "Rename tabCtrl+E"
    )
    expect(screen.getByRole("menuitem", { name: "Reset terminal" }).textContent).toBe(
      "Reset terminalAlt+R"
    )
  })
})
