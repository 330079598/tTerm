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

  it("records distinct commands once", async () => {
    activateFixture(fixture)
    await invoke("list_profiles")
    await invoke("list_profiles")
    await invoke("save_profile")
    expect(invokedCommands()).toEqual(["list_profiles", "save_profile"])
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
