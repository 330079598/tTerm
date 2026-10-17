import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { NOTIFICATION_SOUND_NAMES } from "@/lib/appConfig"
import { TONES, notificationSoundSource, playNotificationSound } from "@/lib/notificationSound"

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }))

const param = () => ({
  setValueAtTime: vi.fn(),
  exponentialRampToValueAtTime: vi.fn(),
})

class FakeAudioContext {
  static oscillators = 0
  static buffersPlayed = 0
  state = "running"
  currentTime = 0
  destination = {}
  createOscillator() {
    FakeAudioContext.oscillators += 1
    return { type: "", frequency: param(), connect: (node: unknown) => node, start() {}, stop() {} }
  }
  createGain() {
    return { gain: param(), connect: (node: unknown) => node }
  }
  createBufferSource() {
    return {
      buffer: null,
      connect: (node: unknown) => node,
      start: () => {
        FakeAudioContext.buffersPlayed += 1
      },
    }
  }
  decodeAudioData(bytes: ArrayBuffer) {
    return Promise.resolve({ length: bytes.byteLength })
  }
}

const settings = (name: string, file = "") =>
  ({ notify_sound: true, notify_sound_name: name, notify_sound_file: file }) as Parameters<
    typeof notificationSoundSource
  >[0]

beforeEach(() => {
  FakeAudioContext.oscillators = 0
  FakeAudioContext.buffersPlayed = 0
  vi.stubGlobal("AudioContext", FakeAudioContext)
  vi.spyOn(console, "warn").mockImplementation(() => {})
  mocks.invoke.mockReset()
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe("notificationSoundSource", () => {
  it("has a tone for every name that is neither the system's nor a file", () => {
    const tones = NOTIFICATION_SOUND_NAMES.filter((name) => name !== "system" && name !== "custom")
    expect(Object.keys(TONES).sort()).toEqual([...tones].sort())
  })

  it("makes no sound when sound is off", () => {
    expect(notificationSoundSource({ ...settings("chime"), notify_sound: false })).toEqual({
      kind: "none",
    })
  })

  it("falls back to the system sound while no file is picked", () => {
    expect(notificationSoundSource(settings("custom", "  "))).toEqual({ kind: "system" })
    expect(notificationSoundSource(settings("custom", "a.mp3"))).toEqual({ kind: "file" })
  })
})

describe("playNotificationSound", () => {
  it("leaves the system sound to the notification", async () => {
    await expect(playNotificationSound(settings("system"))).resolves.toBe(true)
    expect(FakeAudioContext.oscillators).toBe(0)
  })

  it("plays a tone itself", async () => {
    await expect(playNotificationSound(settings("rise"))).resolves.toBe(false)
    expect(FakeAudioContext.oscillators).toBe(TONES.rise.length)
  })

  it("plays tTerm's copy of the sound file", async () => {
    mocks.invoke.mockResolvedValue(new ArrayBuffer(8))
    await expect(playNotificationSound(settings("custom", "a.wav"))).resolves.toBe(false)
    expect(mocks.invoke).toHaveBeenCalledWith("read_notification_sound")
    expect(FakeAudioContext.buffersPlayed).toBe(1)
  })

  it("falls back to the system sound when the copy cannot be read", async () => {
    mocks.invoke.mockRejectedValue("gone")
    await expect(playNotificationSound(settings("custom", "a.wav"))).resolves.toBe(true)
    expect(FakeAudioContext.buffersPlayed).toBe(0)
  })
})
