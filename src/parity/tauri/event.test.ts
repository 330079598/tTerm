import {
  emit,
  expandParityEventName,
  listen,
  once,
  resetParityEvents,
  declareParityEvents,
  missingParityEvents,
} from "@/parity/tauri/event"
import { beforeEach, describe, expect, it } from "vitest"
import { SSH_AUTH_PROMPT_EVENT_PREFIX } from "@/components/TerminalTab/SshAuthPromptDialog"

beforeEach(() => {
  resetParityEvents()
})

describe("parity events", () => {
  it("delivers a payload to a listener", async () => {
    const received: unknown[] = []
    await listen<{ state: string }>("tunnel://status", (event) => received.push(event.payload))
    await emit("tunnel://status", { state: "up" })
    expect(received).toEqual([{ state: "up" }])
  })

  it("stops delivering after unlisten", async () => {
    const received: unknown[] = []
    const unlisten = await listen("app://ready", (event) => received.push(event.payload))
    await emit("app://ready", 1)
    unlisten()
    await emit("app://ready", 2)
    expect(received).toEqual([1])
  })

  it("delivers a one-shot listener once", async () => {
    const received: unknown[] = []
    await once("app://ready", (event) => received.push(event.payload))
    await emit("app://ready", 1)
    await emit("app://ready", 2)
    expect(received).toEqual([1])
  })

  it("ignores events nobody listens to", async () => {
    await expect(emit("app://nobody", null)).resolves.toBeUndefined()
  })

  it("numbers events from one", async () => {
    const ids: number[] = []
    await listen("app://ready", (event) => ids.push(event.id))
    await emit("app://ready", null)
    await emit("app://ready", null)
    expect(ids).toEqual([1, 2])
  })

  it("records listeners that are not declared by the fixture", async () => {
    declareParityEvents(["app://ready"])
    await listen("app://missing", () => {})
    expect(missingParityEvents()).toEqual(["app://missing"])
  })

  it("expands event name templates from fixture variables", () => {
    expect(expandParityEventName("ssh-auth-prompt-{tabId}", { tabId: "tab-1" })).toBe(
      "ssh-auth-prompt-tab-1"
    )
  })

  it("expands a real SSH auth event template to the component listener name", () => {
    expect(
      expandParityEventName(`${SSH_AUTH_PROMPT_EVENT_PREFIX}{tabId}`, { tabId: "tab-1" })
    ).toBe("ssh-auth-prompt-tab-1")
  })
})
