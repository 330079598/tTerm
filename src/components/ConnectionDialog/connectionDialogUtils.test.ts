import { describe, expect, it } from "vitest"

import { certificatePayload } from "@/components/ConnectionDialog/connectionDialogUtils"

describe("certificatePayload", () => {
  it("sends a trimmed certificate only with key authentication", () => {
    expect(certificatePayload("key", " ~/.ssh/id-cert.pub ")).toBe("~/.ssh/id-cert.pub")
    expect(certificatePayload("key", "  ")).toBeUndefined()
    expect(certificatePayload("agent", "~/.ssh/id-cert.pub")).toBeUndefined()
    expect(certificatePayload("password", "~/.ssh/id-cert.pub")).toBeUndefined()
  })
})
