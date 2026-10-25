import { describe, expect, it } from "vitest"

import { detectDistroId, isDistroId, parseOsReport, type LinuxDistributionInfo } from "@/lib/distro"

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

  it("knows FreeBSD by its os-release and macOS by what the monitor reports", () => {
    expect(detectDistroId(info({ id: "freebsd", prettyName: "FreeBSD 14.1-RELEASE" }))).toBe(
      "freebsd"
    )
    expect(detectDistroId(info({ id: "macos", name: "macOS" }))).toBe("macos")
  })

  it("shows generic Linux for an unknown distribution or none", () => {
    expect(detectDistroId(info({ id: "someos", name: "Some OS" }))).toBe("linux")
    expect(detectDistroId(undefined)).toBe("linux")
  })
})

describe("parseOsReport", () => {
  it("reads the WSL launcher's report", () => {
    expect(parseOsReport("tterm-os;ubuntu;debian;Ubuntu")).toEqual(
      info({ id: "ubuntu", idLike: ["debian"], name: "Ubuntu" })
    )
    expect(parseOsReport("tterm-os;rocky;rhel centos fedora;Rocky Linux")?.idLike).toEqual([
      "rhel",
      "centos",
      "fedora",
    ])
  })

  it("names Pop!_OS by the name its id leaves out", () => {
    expect(detectDistroId(parseOsReport("tterm-os;pop;ubuntu debian;Pop!_OS")!)).toBe("popos")
  })

  it("ignores other OSC 777 payloads and empty reports", () => {
    expect(parseOsReport("tterm-agent;claude;done")).toBeNull()
    expect(parseOsReport("notify;title;body")).toBeNull()
    expect(parseOsReport("tterm-os;;;")).toBeNull()
    expect(parseOsReport("tterm-os;ubuntu;debian;Ubuntu;extra")).toBeNull()
  })
})

describe("isDistroId", () => {
  it("accepts only distributions with an icon", () => {
    expect(isDistroId("debian")).toBe(true)
    expect(isDistroId("windows")).toBe(false)
    expect(isDistroId(undefined)).toBe(false)
  })
})
