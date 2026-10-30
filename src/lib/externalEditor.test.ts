import { describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }))

import { editorDisplayName } from "@/lib/externalEditor"

describe("editorDisplayName", () => {
  it("shows a macOS application by its name", () => {
    expect(editorDisplayName("/Applications/Visual Studio Code.app")).toBe("Visual Studio Code")
    expect(editorDisplayName("/Applications/Sublime Text.app/")).toBe("Sublime Text")
    expect(editorDisplayName("TextEdit")).toBe("TextEdit")
  })

  it("drops the folder and suffix of a Windows program", () => {
    expect(editorDisplayName("C:\\Program Files\\Notepad++\\notepad++.exe")).toBe("notepad++")
    expect(editorDisplayName("C:\\Users\\me\\bin\\code.CMD")).toBe("code")
  })

  it("keeps a command name as it is", () => {
    expect(editorDisplayName("/usr/bin/gedit")).toBe("gedit")
    expect(editorDisplayName(" kate ")).toBe("kate")
  })
})
