import { describe, expect, it } from "vitest"

import { agentResumeCwdMatches } from "@/lib/agentResumeCwd"

describe("hook and live shell cwd matching", () => {
  it.each([
    ["C:/Users/Stone/中文 dir", "c:\\users\\stone\\中文 dir"],
    ["\\\\?\\C:\\work", "c:/work"],
    ["\\\\?\\UNC\\HOST\\Share\\work", "\\\\host\\share\\work"],
    ["/work/中文 dir", "/work/中文 dir"],
  ])("matches equivalent directories %s and %s", (hook, shell) => {
    expect(agentResumeCwdMatches(hook, shell)).toBe(true)
  })

  it.each([
    [undefined, "/work"],
    ["/work", undefined],
    ["", ""],
    ["C:/work", "C:/external"],
    ["/Work", "/work"],
    ["/a\\b", "/a/b"],
    ["/work\n", "/work\n"],
  ])("rejects missing, unsafe or different directories %s and %s", (hook, shell) => {
    expect(agentResumeCwdMatches(hook, shell)).toBe(false)
  })
})
