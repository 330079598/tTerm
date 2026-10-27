// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest"

import {
  attachKeymapDispatcher,
  dispatchKeydownEvent,
  type KeymapDispatchEnvironment,
  type KeymapHandler,
} from "@/lib/keymap/dispatcher"
import type { KeymapActionId } from "@/lib/keymap/actions"
import { buildChordIndex, DEFAULT_KEYMAP_CONFIG, resolveEffectiveKeymap } from "@/lib/keymap/keymap"

function createEnv(options?: {
  suppressed?: boolean
  layerOpen?: boolean
  index?: Map<string, KeymapActionId>
  handlers?: Map<KeymapActionId, Set<KeymapHandler>>
  isMac?: boolean
}): KeymapDispatchEnvironment {
  return {
    isDispatchSuppressed: () => options?.suppressed ?? false,
    hasOpenLayer: () => options?.layerOpen ?? false,
    getChordIndex: () => options?.index ?? new Map(),
    getHandlers: () => options?.handlers ?? new Map(),
    isMac: () => options?.isMac ?? false,
  }
}

function keydown(key: string, init: Partial<KeyboardEventInit> = {}): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    key,
    bubbles: true,
    cancelable: true,
    ...init,
  })
}

function eventWithTarget(
  key: string,
  target: EventTarget | null,
  init: Partial<KeyboardEventInit> = {}
) {
  const event = keydown(key, init)
  Object.defineProperty(event, "target", { value: target })
  return event
}

function bindDefaultChords() {
  return buildChordIndex(resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG))
}

describe("dispatchKeydownEvent", () => {
  it("dispatches a bound chord to its handler and consumes the event", () => {
    const handler = vi.fn()
    const handlers = new Map([["terminal.find", new Set([handler])] as const])
    const index = new Map([["mod+f", "terminal.find"] as const])
    const env = createEnv({ handlers, index })

    const event = keydown("f", { ctrlKey: true })
    const result = dispatchKeydownEvent(event, env)

    expect(result).toBe("terminal.find")
    expect(handler).toHaveBeenCalledTimes(1)
    expect(event.defaultPrevented).toBe(true)
  })

  it("gives handlers the physical digit when Option types a symbol on macOS", () => {
    const handler = vi.fn()
    const handlers = new Map([["tabs.switchToNth", new Set([handler])] as const])
    const index = new Map([["alt+1", "tabs.switchToNth"] as const])
    const env = createEnv({ handlers, index, isMac: true })

    const event = keydown("¡", { code: "Digit1", altKey: true })

    expect(dispatchKeydownEvent(event, env)).toBe("tabs.switchToNth")
    expect(handler).toHaveBeenCalledWith(event, expect.objectContaining({ key: "1", alt: true }))
  })

  it("passes unbound chords through untouched", () => {
    const env = createEnv()
    const event = keydown("a", { ctrlKey: true })
    expect(dispatchKeydownEvent(event, env)).toBeNull()
    expect(event.defaultPrevented).toBe(false)
  })

  it("does not consume bound chords when no handler is registered", () => {
    const index = new Map([["mod+f", "terminal.find"] as const])
    const env = createEnv({ index })
    const event = keydown("f", { ctrlKey: true })
    expect(dispatchKeydownEvent(event, env)).toBeNull()
    expect(event.defaultPrevented).toBe(false)
  })

  it("skips actions that disallow editable targets", () => {
    const handler = vi.fn()
    const handlers = new Map([["sftp.selectAll", new Set([handler])] as const])
    const index = new Map([["mod+a", "sftp.selectAll"] as const])
    const env = createEnv({ handlers, index })

    const input = document.createElement("input")
    document.body.appendChild(input)
    const event = eventWithTarget("a", input, { ctrlKey: true })

    expect(dispatchKeydownEvent(event, env)).toBeNull()
    expect(handler).not.toHaveBeenCalled()
    input.remove()
  })

  it("allows editable-target actions when focus is not on an editable surface", () => {
    const handler = vi.fn()
    const handlers = new Map([["sftp.selectAll", new Set([handler])] as const])
    const index = new Map([["mod+a", "sftp.selectAll"] as const])
    const env = createEnv({ handlers, index })

    const event = eventWithTarget("a", document.createElement("div"), { ctrlKey: true })
    expect(dispatchKeydownEvent(event, env)).toBe("sftp.selectAll")
    expect(handler).toHaveBeenCalledTimes(1)
  })

  it("ignores everything while suppressed", () => {
    const handler = vi.fn()
    const handlers = new Map([["terminal.find", new Set([handler])] as const])
    const index = new Map([["mod+f", "terminal.find"] as const])
    const env = createEnv({ suppressed: true, handlers, index })
    expect(dispatchKeydownEvent(keydown("f", { ctrlKey: true }), env)).toBeNull()
  })

  it("ignores everything while a modal is open", () => {
    const handler = vi.fn()
    const handlers = new Map([["terminal.find", new Set([handler])] as const])
    const index = new Map([["mod+f", "terminal.find"] as const])
    const env = createEnv({ layerOpen: true, handlers, index })
    expect(dispatchKeydownEvent(keydown("f", { ctrlKey: true }), env)).toBeNull()
  })

  it("never dispatches Escape", () => {
    const env = createEnv()
    expect(dispatchKeydownEvent(keydown("Escape"), env)).toBeNull()
  })

  it("prefers an explicit Ctrl binding over Mod on macOS", () => {
    const ctrlHandler = vi.fn()
    const modHandler = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["tabs.next", new Set([ctrlHandler])],
      ["terminal.find", new Set([modHandler])],
    ])
    const index = new Map<string, KeymapActionId>([
      ["ctrl+tab", "tabs.next"],
      ["mod+tab", "terminal.find"],
    ])
    const env = createEnv({ handlers, index, isMac: true })

    expect(dispatchKeydownEvent(keydown("Tab", { ctrlKey: true }), env)).toBe("tabs.next")
    expect(ctrlHandler).toHaveBeenCalledOnce()
    expect(modHandler).not.toHaveBeenCalled()
  })

  it("falls back from physical Ctrl to Mod on macOS", () => {
    const handler = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["terminal.zoomIn", new Set([handler])],
    ])
    const index = new Map<string, KeymapActionId>([["mod+=", "terminal.zoomIn"]])
    const env = createEnv({ handlers, index, isMac: true })

    expect(dispatchKeydownEvent(keydown("=", { ctrlKey: true }), env)).toBe("terminal.zoomIn")
    expect(handler).toHaveBeenCalledOnce()
  })

  it("leaves Ctrl and a letter to the terminal on macOS while Cmd and that letter is bound", () => {
    const handler = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["tabs.closeActive", new Set([handler])],
      ["terminal.find", new Set([handler])],
    ])
    const index = new Map<string, KeymapActionId>([
      ["mod+w", "tabs.closeActive"],
      ["mod+f", "terminal.find"],
    ])
    const env = createEnv({ handlers, index, isMac: true })

    const deleteWord = keydown("w", { code: "KeyW", ctrlKey: true })
    expect(dispatchKeydownEvent(deleteWord, env)).toBeNull()
    expect(deleteWord.defaultPrevented).toBe(false)
    expect(dispatchKeydownEvent(keydown("f", { code: "KeyF", ctrlKey: true }), env)).toBeNull()
    expect(handler).not.toHaveBeenCalled()

    expect(dispatchKeydownEvent(keydown("w", { code: "KeyW", metaKey: true }), env)).toBe(
      "tabs.closeActive"
    )
  })

  it("leaves Ctrl+[ and Ctrl+] to the terminal on macOS while Cmd+[ / Cmd+] are bound", () => {
    const handler = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["workspace.focusPrevPane", new Set([handler])],
      ["workspace.focusNextPane", new Set([handler])],
    ])
    const index = new Map<string, KeymapActionId>([
      ["mod+[", "workspace.focusPrevPane"],
      ["mod+]", "workspace.focusNextPane"],
    ])
    const env = createEnv({ handlers, index, isMac: true })

    const escape = keydown("[", { code: "BracketLeft", ctrlKey: true })
    expect(dispatchKeydownEvent(escape, env)).toBeNull()
    expect(escape.defaultPrevented).toBe(false)
    expect(
      dispatchKeydownEvent(keydown("]", { code: "BracketRight", ctrlKey: true }), env)
    ).toBeNull()
    expect(handler).not.toHaveBeenCalled()

    const cmd = keydown("[", { code: "BracketLeft", metaKey: true })
    expect(dispatchKeydownEvent(cmd, env)).toBe("workspace.focusPrevPane")
  })

  it("leaves Ctrl+\\ to the terminal as SIGQUIT on macOS while Cmd+\\ splits", () => {
    const handler = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["workspace.splitRight", new Set([handler])],
    ])
    const index = new Map<string, KeymapActionId>([["mod+\\", "workspace.splitRight"]])
    const env = createEnv({ handlers, index, isMac: true })

    const quit = keydown("\\", { code: "Backslash", ctrlKey: true })
    expect(dispatchKeydownEvent(quit, env)).toBeNull()
    expect(quit.defaultPrevented).toBe(false)
    expect(handler).not.toHaveBeenCalled()

    const cmd = keydown("\\", { code: "Backslash", metaKey: true })
    expect(dispatchKeydownEvent(cmd, env)).toBe("workspace.splitRight")
  })

  it("matches explicit Ctrl bindings on Windows and Linux", () => {
    const handler = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["tabs.next", new Set([handler])],
    ])
    const index = new Map<string, KeymapActionId>([["ctrl+tab", "tabs.next"]])
    const env = createEnv({ handlers, index })

    expect(dispatchKeydownEvent(keydown("Tab", { ctrlKey: true }), env)).toBe("tabs.next")
    expect(handler).toHaveBeenCalledOnce()
  })
})

describe("attachKeymapDispatcher", () => {
  it("wires a capture-phase listener and detaches it", () => {
    const handler = vi.fn()
    const handlers = new Map([["terminal.find", new Set([handler])] as const])
    const index = new Map([["mod+f", "terminal.find"] as const])
    const env = createEnv({ handlers, index })

    const detach = attachKeymapDispatcher(window, env)
    const event = keydown("f", { ctrlKey: true })
    window.dispatchEvent(event)

    expect(handler).toHaveBeenCalledTimes(1)
    expect(event.defaultPrevented).toBe(true)

    detach()
    const second = keydown("f", { ctrlKey: true })
    window.dispatchEvent(second)
    expect(handler).toHaveBeenCalledTimes(1)
    expect(second.defaultPrevented).toBe(false)
  })
})

describe("real default bindings", () => {
  it("resolves tTerm default bindings through the dispatcher", () => {
    const handler = vi.fn()
    const handlers = new Map([["commandLibrary.open", new Set([handler])] as const])
    const index = bindDefaultChords()
    const env = createEnv({ handlers, index })

    expect(dispatchKeydownEvent(keydown("p", { ctrlKey: true, shiftKey: true }), env)).toBe(
      "commandLibrary.open"
    )
  })

  it("dispatches the default new-tab and SFTP shortcuts", () => {
    const newTab = vi.fn()
    const toggleSftp = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["workspace.newTab", new Set([newTab])],
      ["sftp.toggle", new Set([toggleSftp])],
    ])
    const env = createEnv({ handlers, index: bindDefaultChords() })

    expect(dispatchKeydownEvent(keydown("t", { ctrlKey: true, shiftKey: true }), env)).toBe(
      "workspace.newTab"
    )
    expect(dispatchKeydownEvent(keydown("t", { ctrlKey: true }), env)).toBe("sftp.toggle")
    expect(newTab).toHaveBeenCalledOnce()
    expect(toggleSftp).toHaveBeenCalledOnce()
  })

  it("keeps explicit Control defaults distinct from Command on macOS", () => {
    const toggleSftp = vi.fn()
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>([
      ["sftp.toggle", new Set([toggleSftp])],
    ])
    const env = createEnv({ handlers, index: bindDefaultChords(), isMac: true })

    expect(dispatchKeydownEvent(keydown("t", { code: "KeyT", ctrlKey: true }), env)).toBe(
      "sftp.toggle"
    )
    expect(dispatchKeydownEvent(keydown("t", { code: "KeyT", metaKey: true }), env)).toBeNull()
    expect(toggleSftp).toHaveBeenCalledOnce()
  })

  it("lets bound-but-unmounted actions pass through", () => {
    const handlers = new Map<KeymapActionId, Set<KeymapHandler>>()
    const index = bindDefaultChords()
    const env = createEnv({ handlers, index })

    // alt+1 belongs to tabs.switchToNth which has no handler here
    const altEvent = keydown("1", { altKey: true })
    expect(dispatchKeydownEvent(altEvent, env)).toBeNull()
    expect(altEvent.defaultPrevented).toBe(false)
  })
})
