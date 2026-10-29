import React, { useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { platform } from "@tauri-apps/plugin-os"
import { useTranslation } from "react-i18next"

import { NEW_SHELL_KEY } from "@/components/ConnectionDialog/localShellPayload"
import { ShellPathInput } from "@/components/ShellPathInput"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import { buildShellEntries, defaultShellKey, programName, type ShellEntry } from "@/lib/localShells"
import type { TerminalShellProfile } from "@/lib/terminalShells"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface TerminalConnectionFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

// What the list shows on Windows until the shells found are known.
const windowsFallbackTerminalShellProfiles: TerminalShellProfile[] = [
  { shell: "cmd", label: "Command Prompt (cmd)", source: "cmd.exe" },
  { shell: "powershell", label: "Windows PowerShell", source: "powershell.exe" },
  { shell: "pwsh", label: "PowerShell 7 (pwsh)", source: "pwsh.exe" },
  { shell: "wsl", label: "Windows Subsystem for Linux (WSL)", source: "wsl.exe" },
  { shell: "git-bash", label: "Git Bash", source: "bash.exe" },
]

function getFallbackTerminalShellProfiles(): TerminalShellProfile[] {
  try {
    return platform() === "windows" ? windowsFallbackTerminalShellProfiles : []
  } catch {
    return []
  }
}

/** A local terminal's shell: one from the list, or one typed in to add to it. */
export const TerminalConnectionFields: React.FC<TerminalConnectionFieldsProps> = ({
  form,
  setForm,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const [detected, setDetected] = useState<TerminalShellProfile[]>(getFallbackTerminalShellProfiles)

  useEffect(() => {
    let cancelled = false

    invoke<TerminalShellProfile[]>("list_available_terminal_shells")
      .then((profiles) => {
        if (!cancelled && profiles.length > 0) {
          setDetected(profiles)
        }
      })
      .catch((error) => {
        console.warn("Failed to list available terminal shells", error)
      })

    return () => {
      cancelled = true
    }
  }, [])

  const entries = useMemo(() => buildShellEntries(detected, config, t), [config, detected, t])
  const defaultKey = defaultShellKey(config)
  const isNew = form.terminalShellKey === NEW_SHELL_KEY
  const selected = entries.find((entry) => entry.key === form.terminalShellKey)
  // A shell hidden from the lists still shows while it is the one chosen.
  const options = entries.filter((entry) => !entry.hidden || entry === selected)

  const choose = (entry: ShellEntry) =>
    setForm((current) => ({
      ...current,
      terminalShellKey: entry.key,
      terminalShell: entry.choice.shell,
      terminalShellCustomPath: entry.choice.customPath ?? "",
      terminalShellCustomArgs: entry.choice.customArgs ?? "",
      terminalShellId: entry.choice.shellId ?? "",
    }))

  const chooseNew = () =>
    setForm((current) => ({
      ...current,
      terminalShellKey: NEW_SHELL_KEY,
      terminalShell: "custom",
      terminalShellCustomPath: "",
      terminalShellCustomArgs: "",
      terminalShellId: "",
    }))

  return (
    <>
      <div>
        <Label htmlFor="conn-terminal-shell" className="mb-1.5 block">
          {t("connection.terminalShell")}
        </Label>
        <Select
          id="conn-terminal-shell"
          value={form.terminalShellKey}
          onChange={(event) => {
            const key = event.target.value
            const entry = entries.find((candidate) => candidate.key === key)
            if (entry) choose(entry)
            else if (key === NEW_SHELL_KEY) chooseNew()
          }}
        >
          {options.map((entry) => (
            <option key={entry.key} value={entry.key}>
              {entry.key === defaultKey
                ? `${entry.label} (${t("localShells.default")})`
                : entry.label}
            </option>
          ))}
          {/* A default shell that is neither found here nor in the list, kept as it was. */}
          {!selected && !isNew && (
            <option value={form.terminalShellKey}>
              {programName(form.terminalShellCustomPath) || form.terminalShellKey}
            </option>
          )}
          <option value={NEW_SHELL_KEY}>{t("localShells.addNew")}</option>
        </Select>
      </div>

      {isNew && (
        <>
          <div>
            <Label htmlFor="conn-terminal-shell-path" className="mb-1.5 block">
              {t("connection.terminalShellCustomPath")}
            </Label>
            <ShellPathInput
              id="conn-terminal-shell-path"
              value={form.terminalShellCustomPath}
              onChange={(path) =>
                setForm((current) => ({ ...current, terminalShellCustomPath: path }))
              }
              required
            />
          </div>
          <div>
            <Label htmlFor="conn-terminal-shell-args" className="mb-1.5 block">
              {t("connection.terminalShellCustomArgs")}
            </Label>
            <Input
              id="conn-terminal-shell-args"
              value={form.terminalShellCustomArgs}
              onChange={(e) =>
                setForm((current) => ({ ...current, terminalShellCustomArgs: e.target.value }))
              }
              placeholder="--login -i"
            />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <Checkbox
              checked={form.saveShell || form.makeDefaultShell}
              disabled={form.makeDefaultShell}
              onCheckedChange={(saveShell) => setForm((current) => ({ ...current, saveShell }))}
            />
            {t("localShells.saveToList")}
          </label>
        </>
      )}

      {form.terminalShellKey !== defaultKey && (
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={form.makeDefaultShell}
            onCheckedChange={(makeDefaultShell) =>
              setForm((current) => ({ ...current, makeDefaultShell }))
            }
          />
          {t("localShells.makeDefaultShell")}
        </label>
      )}
    </>
  )
}
