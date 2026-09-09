import { describe, expect, it } from "vitest"

import { KEYMAP_ACTION_IDS } from "@/lib/keymap/actions"
import {
  buildChordIndex,
  findKeymapConflicts,
  getCliConflictHint,
  normalizeKeymap,
  resolveEffectiveKeymap,
  DEFAULT_KEYMAP_CONFIG,
  type EffectiveKeymap,
} from "@/lib/keymap/keymap"

describe("resolveEffectiveKeymap", () => {
  it("provides a tTerm default for every action", () => {
    const effective = resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG)
    for (const id of KEYMAP_ACTION_IDS) {
      expect(effective[id]).not.toBeUndefined()
    }
    expect(effective["workspace.splitRight"]).toEqual(["mod+\\"])
    expect(effective["workspace.newTab"]).toEqual(["ctrl+shift+t"])
    expect(effective["sftp.toggle"]).toEqual(["ctrl+t"])
    expect(effective["commandLibrary.open"]).toEqual(["mod+shift+p"])
    expect(effective["tabs.next"]).toEqual(["ctrl+tab"])
  })

  it("supports multiple chords for one action (nth tab switching)", () => {
    const effective = resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG)
    expect(effective["tabs.switchToNth"]).toHaveLength(9)
    expect(effective["tabs.switchToNth"]![0]).toBe("alt+1")
  })

  it("lets an explicit override replace the default binding", () => {
    const effective = resolveEffectiveKeymap({
      bindings: { "terminal.find": ["mod+j"] },
    })
    expect(effective["terminal.find"]).toEqual(["mod+j"])
  })

  it("lets an explicit null unbind an action", () => {
    const effective = resolveEffectiveKeymap({
      bindings: { "terminal.find": null },
    })
    expect(effective["terminal.find"]).toBeNull()
  })

  it("falls back to the default when an override is unparseable", () => {
    const effective = resolveEffectiveKeymap({
      bindings: { "terminal.find": ["not a chord"] },
    })
    expect(effective["terminal.find"]).toEqual(["mod+f"])
  })
})

describe("findKeymapConflicts", () => {
  it("reports chords bound to multiple actions", () => {
    const effective: EffectiveKeymap = {
      ...resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG),
      "terminal.clear": ["mod+f"],
    }
    const conflicts = findKeymapConflicts(effective)
    expect(conflicts.get("mod+f")).toEqual(["terminal.find", "terminal.clear"])
  })

  it("returns empty for the tTerm defaults", () => {
    expect(findKeymapConflicts(resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG)).size).toBe(0)
  })

  it("does not flag multiple chords on a single action", () => {
    const effective = resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG)
    expect(findKeymapConflicts(effective).size).toBe(0)
  })
})

describe("buildChordIndex", () => {
  it("maps each chord to its action", () => {
    const index = buildChordIndex(resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG))
    expect(index.get("mod+f")).toBe("terminal.find")
    expect(index.get("ctrl+shift+t")).toBe("workspace.newTab")
    expect(index.get("ctrl+t")).toBe("sftp.toggle")
    expect(index.get("alt+1")).toBe("tabs.switchToNth")
    expect(index.get("mod+shift+p")).toBe("commandLibrary.open")
  })
})

describe("normalizeKeymap", () => {
  it("returns defaults for non-object input", () => {
    expect(normalizeKeymap(undefined)).toEqual(DEFAULT_KEYMAP_CONFIG)
    expect(normalizeKeymap(null)).toEqual(DEFAULT_KEYMAP_CONFIG)
  })

  it("keeps valid bindings and drops invalid ones", () => {
    const normalized = normalizeKeymap({
      bindings: {
        "terminal.find": ["mod+j"],
        "terminal.clear": "bad chord",
        "editor.save": null,
        "unknown.action": ["mod+x"],
      },
    })
    expect(normalized.bindings["terminal.find"]).toEqual(["mod+j"])
    expect(normalized.bindings["terminal.clear"]).toBeUndefined()
    expect(normalized.bindings["editor.save"]).toBeNull()
    expect(Object.prototype.hasOwnProperty.call(normalized.bindings, "unknown.action")).toBe(false)
  })

  it("drops empty and invalid overrides but preserves explicit unbinding", () => {
    const normalized = normalizeKeymap({
      bindings: {
        "terminal.find": [],
        "terminal.clear": ["not a chord"],
        "editor.save": null,
      },
    })

    expect(normalized.bindings["terminal.find"]).toBeUndefined()
    expect(normalized.bindings["terminal.clear"]).toBeUndefined()
    expect(normalized.bindings["editor.save"]).toBeNull()
  })

  it("canonicalizes chord serializations", () => {
    const normalized = normalizeKeymap({
      bindings: { "terminal.find": ["SHIFT+mod+J"] },
    })
    expect(normalized.bindings["terminal.find"]).toEqual(["mod+shift+j"])
  })
})

describe("getCliConflictHint", () => {
  it("returns hint ids for known readline/tmux chords", () => {
    expect(getCliConflictHint("mod+s")).toBe("xoff")
    expect(getCliConflictHint("mod+b")).toBe("tmuxPrefix")
    expect(getCliConflictHint("mod+a")).toBe("readlineHome")
    expect(getCliConflictHint("ctrl+s", true)).toBe("xoff")
    expect(getCliConflictHint("mod+s", true)).toBeUndefined()
  })

  it("returns undefined for unknown chords", () => {
    expect(getCliConflictHint("mod+shift+k")).toBeUndefined()
  })
})
