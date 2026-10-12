import { describe, expect, it } from "vitest"

import { parseOsc52Copy } from "@/lib/osc52"

const encode = (text: string) => btoa(String.fromCharCode(...new TextEncoder().encode(text)))

describe("parseOsc52Copy", () => {
  it("decodes the text a program copies", () => {
    expect(parseOsc52Copy(`c;${encode("hello world")}`)).toBe("hello world")
  })

  it("decodes UTF-8 text", () => {
    expect(parseOsc52Copy(`c;${encode("你好，vim\n")}`)).toBe("你好，vim\n")
  })

  it("accepts any selection target, or none", () => {
    expect(parseOsc52Copy(`;${encode("a")}`)).toBe("a")
    expect(parseOsc52Copy(`p;${encode("a")}`)).toBe("a")
    expect(parseOsc52Copy(`s0;${encode("a")}`)).toBe("a")
  })

  it("never answers a clipboard query", () => {
    expect(parseOsc52Copy("c;?")).toBeNull()
  })

  it("ignores a request to clear the clipboard", () => {
    expect(parseOsc52Copy("c;")).toBeNull()
  })

  it("ignores malformed sequences", () => {
    expect(parseOsc52Copy("aGVsbG8=")).toBeNull()
    expect(parseOsc52Copy("x;aGVsbG8=")).toBeNull()
    expect(parseOsc52Copy("c;not base64!")).toBeNull()
  })
})
