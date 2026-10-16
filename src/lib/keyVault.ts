/** A key in the SSH key vault, as `list_vault_keys` returns it. */
export interface VaultKeySummary {
  id: string
  name: string
  /** The key type, such as `ssh-ed25519`. */
  algorithm: string
  /** `SHA256:…`, as `ssh-keygen -l` shows it. */
  fingerprint: string
  /** The public key line for `authorized_keys`. */
  publicKey: string
  /** The key itself is protected by a passphrase. */
  encrypted: boolean
  /** Its passphrase is kept with it, so connecting asks for none. */
  passphraseSaved: boolean
  hasCertificate: boolean
  /** When the certificate expires (unix seconds); null for one that never does. */
  certificateValidBefore: number | null
  /** Unix milliseconds. */
  createdAt: number
  /** The profiles that sign in with the key. */
  usedBy: string[]
}

/** What `import_vault_key` takes. */
export interface ImportVaultKeyInput {
  path: string
  name?: string
  passphrase?: string
  rememberPassphrase?: boolean
  certificatePath?: string
}

/** Matches `REFERENCE_PREFIX` in src-tauri/src/ssh/key_vault/mod.rs. */
const REFERENCE_PREFIX = "tterm-key:"

/** The private key path that signs in with vault key `id`. */
export function vaultKeyReference(id: string): string {
  return `${REFERENCE_PREFIX}${id}`
}

/** The vault key id a private key path names, or null for a key file. */
export function vaultKeyId(path: string | null | undefined): string | null {
  const trimmed = path?.trim() ?? ""
  if (!trimmed.startsWith(REFERENCE_PREFIX)) return null
  const id = trimmed.slice(REFERENCE_PREFIX.length).trim()
  return id || null
}

export function isVaultKeyPath(path: string | null | undefined): boolean {
  return vaultKeyId(path) !== null
}

/** The fingerprint shortened for a list, keeping its `SHA256:` start and end. */
export function shortFingerprint(fingerprint: string): string {
  // Without a hash name the colon is not found and the whole text is the digest.
  const digestStart = fingerprint.indexOf(":") + 1
  const hash = fingerprint.slice(0, digestStart)
  const digest = fingerprint.slice(digestStart)
  return digest.length > 12 ? `${hash}${digest.slice(0, 6)}…${digest.slice(-6)}` : fingerprint
}
