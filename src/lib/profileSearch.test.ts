import { describe, expect, it } from "vitest"

import { profileSearchFields } from "@/lib/profileSearch"
import { matchesQuery } from "@/lib/quickConnect"
import { DEFAULT_SERIAL_SETTINGS } from "@/lib/serial"
import type { SavedProfile } from "@/types/tab"

const profile = (overrides: Partial<SavedProfile> = {}): SavedProfile => ({
  id: "p1",
  name: "web-01",
  group: "Production",
  connection_type: "ssh",
  host: "10.0.0.5",
  port: 22,
  username: "root",
  remember_password: true,
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
  ...overrides,
})

describe("profileSearchFields", () => {
  it("finds a connection by words from different fields in any order", () => {
    const fields = profileSearchFields(profile())
    expect(matchesQuery("root web", fields)).toBe(true)
    expect(matchesQuery("prod 10.0.0", fields)).toBe(true)
    expect(matchesQuery("web admin", fields)).toBe(false)
  })

  it("finds a serial connection by its port", () => {
    const fields = profileSearchFields(
      profile({
        connection_type: "serial",
        host: undefined,
        username: undefined,
        serial: { ...DEFAULT_SERIAL_SETTINGS, port: "COM3" },
      })
    )
    expect(matchesQuery("com3", fields)).toBe(true)
  })
})
