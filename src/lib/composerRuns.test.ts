import { describe, expect, it } from "vitest"

import type { PromptEvent, ShellPromptState } from "@/components/TerminalTab/commandMarks"
import { registerComposerTarget } from "@/lib/commandComposer"
import {
  continueComposerRun,
  dismissComposerRun,
  startComposerRun,
  stopComposerRun,
} from "@/lib/composerRuns"

/** A terminal at its prompt that records what it is sent. */
function fakeTerminal(tabId: string, state: ShellPromptState = "prompt") {
  const sent: string[] = []
  let listener: ((event: PromptEvent) => void) | null = null
  const unregister = registerComposerTarget(tabId, {
    input: (data) => sent.push(data),
    bracketedPasteMode: () => true,
    promptState: () => state,
    onPrompt: (next) => {
      listener = next
      return () => {
        listener = null
      }
    },
  })
  return {
    sent,
    prompt: (exitCode?: number) => listener?.({ kind: "prompt", exitCode }),
    end: () => listener?.({ kind: "ended" }),
    following: () => listener !== null,
    unregister,
  }
}

describe("composer runs in a tab", () => {
  it("sends the next line only once the shell is back at its prompt", () => {
    const terminal = fakeTerminal("run-1")
    expect(startComposerRun("run-1", ["cd /tmp", "ls"])).toBe("started")
    expect(terminal.sent).toEqual(["cd /tmp\r"])
    terminal.prompt(0)
    expect(terminal.sent).toEqual(["cd /tmp\r", "ls\r"])
    terminal.prompt(0)
    expect(terminal.following()).toBe(false)
    dismissComposerRun("run-1")
    terminal.unregister()
  })

  it("waits on a failed line until told to go on", () => {
    const terminal = fakeTerminal("run-2")
    startComposerRun("run-2", ["false", "echo after"])
    terminal.prompt(1)
    expect(terminal.sent).toEqual(["false\r"])
    // A second run can't start while this one waits.
    expect(startComposerRun("run-2", ["echo other"])).toBe("busy")
    continueComposerRun("run-2")
    expect(terminal.sent).toEqual(["false\r", "echo after\r"])
    stopComposerRun("run-2")
    dismissComposerRun("run-2")
    terminal.unregister()
  })

  it("stops when the session's marks end", () => {
    const terminal = fakeTerminal("run-3")
    startComposerRun("run-3", ["sleep 9", "echo after"])
    terminal.end()
    terminal.prompt(0)
    expect(terminal.sent).toEqual(["sleep 9\r"])
    dismissComposerRun("run-3")
    terminal.unregister()
  })

  it("says why it doesn't start where the shell isn't at a marked prompt", () => {
    const unmarked = fakeTerminal("run-4", "unmarked")
    expect(startComposerRun("run-4", ["ls"])).toBe("unmarked")
    unmarked.unregister()
    const busy = fakeTerminal("run-4", "busy")
    expect(startComposerRun("run-4", ["ls"])).toBe("busy")
    expect(busy.sent).toEqual([])
    busy.unregister()
  })
})
