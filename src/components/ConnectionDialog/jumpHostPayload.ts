import { normalizeSshAuthMethod } from "@/lib/profileConnections"
import type { JumpHostConnection, SavedJumpHost } from "@/types/tab"
import { ConnectionForm } from "@/components/ConnectionDialog/types"

export const getJumpHostPasswordLookupKey = (
  jump: Pick<JumpHostConnection, "host" | "port" | "username">
) => `${jump.host.trim()}:${jump.port}:${jump.username.trim()}`

export const getJumpHostErrorKey = (jumpId: string, field: string) => `${jumpId}:${field}`

export const getJumpHostFieldElementId = (jumpId: string, field: string) => {
  const fieldIds: Record<string, string> = {
    host: "jump-host",
    port: "jump-port",
    username: "jump-username",
    privateKeyPath: "jump-key-path",
  }
  return `${fieldIds[field]}-${jumpId}`
}

export const getJumpHostValidationErrors = (form: ConnectionForm): Record<string, string> => {
  const errors: Record<string, string> = {}
  if (!form.useJumpHost) return errors

  for (const [index, jump] of form.jumpHosts.entries()) {
    const label = `Jump host #${index + 1}`
    if (!jump.host.trim())
      errors[getJumpHostErrorKey(jump.id, "host")] = `${label} host is required.`
    if (!jump.username.trim()) {
      errors[getJumpHostErrorKey(jump.id, "username")] = `${label} username is required.`
    }
    if (!Number.isInteger(jump.port) || jump.port < 1 || jump.port > 65535) {
      errors[getJumpHostErrorKey(jump.id, "port")] = `${label} port must be between 1 and 65535.`
    }
    if (jump.authMethod === "key" && !jump.privateKeyPath.trim()) {
      errors[getJumpHostErrorKey(jump.id, "privateKeyPath")] =
        `${label} private key path is required.`
    }
  }
  return errors
}

export const getJumpHostValidationError = (form: ConnectionForm): string | null => {
  if (!form.useJumpHost) {
    return null
  }

  if (form.jumpHosts.length === 0) return "At least one jump host is required."

  return Object.values(getJumpHostValidationErrors(form))[0] ?? null
}

export function buildJumpHostsPayload(
  form: ConnectionForm,
  keyCase: "snake",
  savedJumpPasswordKeys?: Set<string>,
  preserveWhenDisabled?: boolean
): SavedJumpHost[] | undefined
export function buildJumpHostsPayload(
  form: ConnectionForm,
  keyCase: "camel",
  savedJumpPasswordKeys?: Set<string>
): JumpHostConnection[] | undefined
export function buildJumpHostsPayload(
  form: ConnectionForm,
  keyCase: "camel" | "snake",
  savedJumpPasswordKeys: Set<string> = new Set(),
  preserveWhenDisabled = false
): SavedJumpHost[] | JumpHostConnection[] | undefined {
  if (form.jumpHosts.length === 0 || (!form.useJumpHost && !preserveWhenDisabled)) {
    return undefined
  }

  if (keyCase === "snake") {
    return form.jumpHosts.map((jump) => {
      const authMethod = normalizeSshAuthMethod(jump.authMethod)
      const privateKeyPath = authMethod === "key" ? jump.privateKeyPath || undefined : undefined
      const privateKeyPassphrase =
        authMethod === "key" ? jump.privateKeyPassphrase || undefined : undefined
      return {
        host: jump.host.trim(),
        port: jump.port,
        username: jump.username.trim(),
        auth_method: authMethod,
        password: authMethod === "password" ? jump.password || undefined : undefined,
        private_key_path: privateKeyPath,
        private_key_passphrase: privateKeyPassphrase,
      }
    })
  }

  return form.jumpHosts.map((jump) => {
    const authMethod = normalizeSshAuthMethod(jump.authMethod)
    const savedPasswordAvailable = savedJumpPasswordKeys.has(getJumpHostPasswordLookupKey(jump))
    return {
      host: jump.host.trim(),
      port: jump.port,
      username: jump.username.trim(),
      password:
        authMethod === "password" && (!savedPasswordAvailable || jump.password.length > 0)
          ? jump.password || undefined
          : undefined,
      authMethod,
      privateKeyPath: authMethod === "key" ? jump.privateKeyPath || undefined : undefined,
      privateKeyPassphrase:
        authMethod === "key" ? jump.privateKeyPassphrase || undefined : undefined,
    }
  })
}
