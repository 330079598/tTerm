import { describe, expect, it } from "vitest"

import {
  UserVerificationCanceledError,
  isVerificationCanceled,
  normalizeVerificationMethod,
  verificationRequired,
} from "@/lib/userVerification"

describe("verificationRequired", () => {
  it("reads the method from the backend error", () => {
    expect(verificationRequired("USER_VERIFICATION_REQUIRED:system")).toBe("system")
    expect(verificationRequired("USER_VERIFICATION_REQUIRED:masterPassword")).toBe("masterPassword")
    expect(verificationRequired(new Error("USER_VERIFICATION_REQUIRED:system"))).toBe("system")
  })

  it("ignores other errors", () => {
    expect(verificationRequired("Saved passwords are locked. Unlock them first.")).toBeNull()
    expect(verificationRequired("USER_VERIFICATION_REQUIRED:none")).toBeNull()
    expect(verificationRequired(undefined)).toBeNull()
  })
})

describe("isVerificationCanceled", () => {
  it("recognizes only the cancel error", () => {
    expect(isVerificationCanceled(new UserVerificationCanceledError("canceled"))).toBe(true)
    expect(isVerificationCanceled(new Error("canceled"))).toBe(false)
  })
})

describe("normalizeVerificationMethod", () => {
  it("falls back to none", () => {
    expect(normalizeVerificationMethod("system")).toBe("system")
    expect(normalizeVerificationMethod("masterPassword")).toBe("masterPassword")
    expect(normalizeVerificationMethod("hello")).toBe("none")
    expect(normalizeVerificationMethod(undefined)).toBe("none")
  })
})
