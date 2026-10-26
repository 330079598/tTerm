import type { ConnectionForm } from "@/components/ConnectionDialog/types"
import {
  KEEPALIVE_COUNT_RANGE,
  KEEPALIVE_INTERVAL_RANGE,
} from "@/components/ConnectionDialog/types"
import {
  encodingPayload,
  highlightSetPayload,
  sessionLogPayload,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import { normalizeSerialSettings } from "@/lib/serial"
import type { SavedProfile, Tab } from "@/types/tab"

/** A serial connection as saved, with only what a serial profile keeps. */
export function buildSerialProfile(
  form: ConnectionForm,
  identity: Pick<SavedProfile, "id" | "name" | "group_path">
): SavedProfile {
  return {
    ...identity,
    connection_type: "serial",
    remember_password: false,
    keepalive_interval_secs: KEEPALIVE_INTERVAL_RANGE.fallback,
    keepalive_count_max: KEEPALIVE_COUNT_RANGE.fallback,
    terminal_theme: form.terminalTheme || undefined,
    encoding: encodingPayload(form.encoding),
    session_log: sessionLogPayload(form.sessionLog),
    highlight_set: highlightSetPayload(form.highlightSet),
    serial: normalizeSerialSettings(form.serial),
  }
}

/** What a serial tab connects with. */
export function buildSerialConnection(
  form: ConnectionForm,
  title: string,
  profileId: string | undefined
): NonNullable<Tab["connection"]> {
  return {
    type: "serial",
    profileId,
    profileName: title,
    terminalTheme: form.terminalTheme || undefined,
    encoding: encodingPayload(form.encoding),
    sessionLog: sessionLogPayload(form.sessionLog),
    highlightSet: highlightSetPayload(form.highlightSet),
    serial: normalizeSerialSettings(form.serial),
  }
}
