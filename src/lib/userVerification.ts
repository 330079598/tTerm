import { toErrorMessage } from "@/lib/utils"

/** What a verification is for; the backend keeps the grants apart. */
export type VerificationPurpose = "reveal" | "sensitive"
/** How this device confirms it is the user. */
export type VerificationMethod = "system" | "masterPassword" | "none"

/** Matches `VERIFICATION_REQUIRED` in src-tauri/src/ssh/secret_store/verification.rs. */
const VERIFICATION_REQUIRED = "USER_VERIFICATION_REQUIRED:"
/** Matches `CANCELED` in src-tauri/src/ssh/secret_store/os_verifier.rs. */
export const SYSTEM_VERIFICATION_CANCELED = "Verification was canceled."

/** The method the backend asks for, or null when `error` is something else. */
export function verificationRequired(error: unknown): Exclude<VerificationMethod, "none"> | null {
  const message = toErrorMessage(error)
  if (!message.startsWith(VERIFICATION_REQUIRED)) return null
  const method = message.slice(VERIFICATION_REQUIRED.length)
  return method === "system" || method === "masterPassword" ? method : null
}

/** The user closed the verification prompt; nothing needs reporting. */
export class UserVerificationCanceledError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "UserVerificationCanceledError"
  }
}

export function isVerificationCanceled(error: unknown): boolean {
  return error instanceof UserVerificationCanceledError
}

export function normalizeVerificationMethod(value: unknown): VerificationMethod {
  return value === "system" || value === "masterPassword" ? value : "none"
}
