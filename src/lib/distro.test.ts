import { describe, expect, it } from "vitest"

import { detectDistroId, type LinuxDistributionInfo } from "@/lib/distro"

function info(fields: Partial<LinuxDistributionInfo>): LinuxDistributionInfo {
  return { id: "", idLike: [], name: "", prettyName: "", ...fields }
}

describe("detectDistroId", () => {
  it("knows a distribution by its id", () => {
    expect(detectDistroId(info({ id: "ubuntu", idLike: ["debian"] }))).toBe("ubuntu")
    expect(detectDistroId(info({ id: "amzn" }))).toBe("amazon")
  })

  it("prefers a derivative over the distribution it is based on", () => {
    expect(detectDistroId(info({ id: "linuxmint", idLike: ["ubuntu", "debian"] }))).toBe(
      "linuxmint"
    )
    expect(detectDistroId(info({ id: "manjaro", idLike: ["arch"] }))).toBe("manjaro")
  })

  it("falls back to what a distribution is like", () => {
    expect(detectDistroId(info({ id: "someos", idLike: ["rhel", "fedora"] }))).toBe("rhel")
  })

  it("shows generic Linux for an unknown distribution or none", () => {
    expect(detectDistroId(info({ id: "someos", name: "Some OS" }))).toBe("linux")
    expect(detectDistroId(undefined)).toBe("linux")
  })
})
