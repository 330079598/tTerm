// @vitest-environment jsdom
import {
  activateFixture,
  Channel,
  invoke,
  invokedCommands,
  isTauri,
  resetParityCore,
  sendChannelMessage,
} from "@/parity/tauri/core"
import { beforeEach, describe, expect, it } from "vitest"

const fixture = {
  commands: {
    list_profiles: [{ id: "p1", name: "prod-web-01" }],
    save_profile: true,
  },
}

beforeEach(() => {
  resetParityCore()
})

describe("parity invoke", () => {
  it("answers a command from the active fixture", async () => {
    activateFixture(fixture)
    await expect(invoke("list_profiles")).resolves.toEqual([{ id: "p1", name: "prod-web-01" }])
  })

  it("returns a clone so a screen cannot change the fixture", async () => {
    activateFixture(fixture)
    const first = await invoke<{ id: string; name: string }[]>("list_profiles")
    first[0].name = "changed"
    await expect(invoke("list_profiles")).resolves.toEqual([{ id: "p1", name: "prod-web-01" }])
  })

  it("rejects a command without a fixture and names the known commands", async () => {
    activateFixture(fixture)
    await expect(invoke("delete_everything")).rejects.toThrow(
      'no fixture for "delete_everything"; fixtures provide: list_profiles, save_profile'
    )
  })

  it("rejects when no fixture is active", async () => {
    await expect(invoke("list_profiles")).rejects.toThrow("no fixture is active")
  })

  it("records commands with no fixture, including caught and repeated calls", async () => {
    activateFixture(fixture)
    await expect(invoke("delete_everything")).rejects.toThrow("no fixture")
    await expect(invoke("delete_everything")).rejects.toThrow("no fixture")
    await expect(invoke("another_missing")).rejects.toThrow("no fixture")
    expect(window.__PARITY_MISSING__).toEqual(["delete_everything", "another_missing"])
    expect(invokedCommands()).toEqual(["another_missing", "delete_everything"])
  })

  it("records a missing command when no fixture is active", async () => {
    await expect(invoke("list_profiles")).rejects.toThrow("no fixture is active")
    expect(window.__PARITY_MISSING__).toEqual(["list_profiles"])
  })

  it("rejects with the fixture's error text and does not report it missing", async () => {
    activateFixture({ commands: { save_profile: { $reject: "permission denied" } } })
    await expect(invoke("save_profile")).rejects.toThrow("permission denied")
    expect(window.__PARITY_MISSING__).toEqual([])
  })

  it("never settles a pending fixture", async () => {
    activateFixture({ commands: { slow: { $pending: true } } })
    const pending = invoke<string>("slow")
    const settled = await Promise.race([
      pending.then(() => "settled"),
      new Promise<string>((resolve) => setTimeout(() => resolve("pending"), 0)),
    ])
    expect(settled).toBe("pending")
    expect(window.__PARITY_MISSING__).toEqual([])
  })

  it("matches an argument case exactly", async () => {
    activateFixture({
      commands: {
        by_args: [
          { args: { id: "a" }, value: "A" },
          { args: { id: "b" }, value: "B" },
        ],
      },
    })
    await expect(invoke("by_args", { id: "b" })).resolves.toBe("B")
    await expect(invoke("by_args", { id: "a", extra: true })).rejects.toThrow(
      'no args fixture for "by_args"'
    )
    expect(window.__PARITY_MISSING__).toEqual(["by_args"])
  })

  it("leaves an empty array as a normal value", async () => {
    activateFixture({ commands: { list_profiles: [] } })
    await expect(invoke("list_profiles")).resolves.toEqual([])
  })

  it("records distinct commands once", async () => {
    activateFixture(fixture)
    await invoke("list_profiles")
    await invoke("list_profiles")
    await invoke("save_profile")
    expect(invokedCommands()).toEqual(["list_profiles", "save_profile"])
  })

  it("clears the missing list when reset", async () => {
    await expect(invoke("list_profiles")).rejects.toThrow("no fixture is active")
    resetParityCore()
    expect(window.__PARITY_MISSING__).toEqual([])
  })

  it("reports that it emulates Tauri", () => {
    expect(isTauri()).toBe(true)
  })
})

describe("parity Channel", () => {
  it("delivers a backend message to its handler", () => {
    const received: string[] = []
    const channel = new Channel<string>((message) => received.push(message))
    sendChannelMessage(channel.id, "first")
    sendChannelMessage(channel.id, "second")
    expect(received).toEqual(["first", "second"])
  })

  it("uses a new id per channel", () => {
    activateFixture(fixture)
    expect(new Channel().id).not.toBe(new Channel().id)
  })

  it("serialises to its id", () => {
    const channel = new Channel()
    expect(JSON.stringify({ channel })).toBe(`{"channel":"${channel.id}"}`)
  })

  it("throws for an unknown channel id", () => {
    expect(() => sendChannelMessage(999, "lost")).toThrow("no channel with id 999")
  })
})
