import { describe, expect, it } from "vitest"

import { isVaultKeyPath, shortFingerprint, vaultKeyId, vaultKeyReference } from "@/lib/keyVault"

describe("vault key references", () => {
  it("round-trips an id through the private key path", () => {
    expect(vaultKeyReference("abc")).toBe("tterm-key:abc")
    expect(vaultKeyId(vaultKeyReference("abc"))).toBe("abc")
  })

  it("reads the id as the backend does", () => {
    expect(vaultKeyId("  tterm-key: abc ")).toBe("abc")
    expect(vaultKeyId("tterm-key:")).toBeNull()
    expect(vaultKeyId("~/.ssh/id_ed25519")).toBeNull()
    expect(vaultKeyId("C:\\keys\\tterm-key:abc")).toBeNull()
    expect(vaultKeyId(undefined)).toBeNull()
    expect(isVaultKeyPath("tterm-key:abc")).toBe(true)
    expect(isVaultKeyPath("/keys/id")).toBe(false)
  })
})

describe("shortFingerprint", () => {
  it("keeps the hash name and both ends of the digest", () => {
    expect(shortFingerprint("SHA256:abcdefghijklmnopqrstuvwxyz")).toBe("SHA256:abcdef…uvwxyz")
  })

  it("leaves a short fingerprint as it is", () => {
    expect(shortFingerprint("SHA256:abc")).toBe("SHA256:abc")
    expect(shortFingerprint("")).toBe("")
  })
})
