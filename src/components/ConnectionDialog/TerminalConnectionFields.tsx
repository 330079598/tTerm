import React, { useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { platform } from "@tauri-apps/plugin-os"
import { useTranslation } from "react-i18next"

import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { TerminalShellType } from "@/types/tab"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface TerminalShellProfile {
  shell: TerminalShellType
  label: string
  source: string
}

type TerminalShellOption = TerminalShellProfile & {
  value: string
}

interface TerminalConnectionFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

const windowsFallbackTerminalShellProfiles: TerminalShellProfile[] = [
  { shell: "auto", label: "Auto (recommended)", source: "tTerm default" },
  { shell: "cmd", label: "Command Prompt (cmd)", source: "cmd.exe" },
  { shell: "powershell", label: "Windows PowerShell", source: "powershell.exe" },
  { shell: "pwsh", label: "PowerShell 7 (pwsh)", source: "pwsh.exe" },
  { shell: "wsl", label: "Windows Subsystem for Linux (WSL)", source: "wsl.exe" },
  { shell: "git-bash", label: "Git Bash", source: "bash.exe" },
  { shell: "custom", label: "Custom executable", source: "manual path" },
]

const unixFallbackTerminalShellProfiles: TerminalShellProfile[] = [
  { shell: "auto", label: "Auto (recommended)", source: "tTerm default" },
  { shell: "custom", label: "Custom shell", source: "$SHELL" },
]

function getFallbackTerminalShellProfiles(): TerminalShellProfile[] {
  try {
    return platform() === "windows"
      ? windowsFallbackTerminalShellProfiles
      : unixFallbackTerminalShellProfiles
  } catch {
    return unixFallbackTerminalShellProfiles
  }
}

const terminalShellTranslationKeys: Record<TerminalShellType, string> = {
  auto: "auto",
  cmd: "cmd",
  powershell: "powershell",
  pwsh: "pwsh",
  wsl: "wsl",
  "git-bash": "gitBash",
  custom: "custom",
}

function toShellOption(profile: TerminalShellProfile): TerminalShellOption {
  return {
    ...profile,
    value: profile.shell === "custom" ? `custom:${profile.source}` : profile.shell,
  }
}

export const TerminalConnectionFields: React.FC<TerminalConnectionFieldsProps> = ({
  form,
  setForm,
}) => {
  const { t } = useTranslation()
  const [fallbackShellProfiles] = useState<TerminalShellProfile[]>(getFallbackTerminalShellProfiles)
  const [shellProfiles, setShellProfiles] = useState<TerminalShellProfile[]>(fallbackShellProfiles)

  useEffect(() => {
    let cancelled = false

    invoke<TerminalShellProfile[]>("list_available_terminal_shells")
      .then((profiles) => {
        if (!cancelled && profiles.length > 0) {
          setShellProfiles(profiles)
        }
      })
      .catch((error) => {
        console.warn("Failed to list available terminal shells", error)
      })

    return () => {
      cancelled = true
    }
  }, [])

  const visibleShellOptions = useMemo(() => {
    if (shellProfiles.some((profile) => profile.shell === form.terminalShell)) {
      return shellProfiles.map(toShellOption)
    }

    const selectedProfile = fallbackShellProfiles.find(
      (profile) => profile.shell === form.terminalShell
    )
    return (selectedProfile ? [...shellProfiles, selectedProfile] : shellProfiles).map(
      toShellOption
    )
  }, [fallbackShellProfiles, form.terminalShell, shellProfiles])

  const selectedShellValue =
    form.terminalShell === "custom" && form.terminalShellCustomPath
      ? `custom:${form.terminalShellCustomPath}`
      : form.terminalShell

  return (
    <>
      <div>
        <Label htmlFor="conn-terminal-shell" className="mb-1.5 block">
          {t("connection.terminalShell")}
        </Label>
        <Select
          id="conn-terminal-shell"
          value={selectedShellValue}
          onChange={(e) => {
            const selectedValue = e.target.value
            const selectedProfile = visibleShellOptions.find(
              (profile) => profile.value === selectedValue
            )
            const isDetectedCustomShell =
              selectedProfile?.shell === "custom" &&
              selectedProfile.source !== "manual path" &&
              selectedProfile.source !== "$SHELL"

            setForm((current) => ({
              ...current,
              terminalShell: selectedProfile?.shell ?? (selectedValue as TerminalShellType),
              terminalShellCustomPath: isDetectedCustomShell
                ? selectedProfile.source
                : current.terminalShellCustomPath,
            }))
          }}
        >
          {visibleShellOptions.map((profile) => (
            <option key={profile.value} value={profile.value}>
              {profile.shell === "custom"
                ? profile.label
                : t(
                    `connection.terminalShellOptions.${terminalShellTranslationKeys[profile.shell]}`,
                    profile.label
                  )}
            </option>
          ))}
        </Select>
      </div>

      {form.terminalShell === "custom" && (
        <>
          <div>
            <Label htmlFor="conn-terminal-shell-path" className="mb-1.5 block">
              {t("connection.terminalShellCustomPath")}
            </Label>
            <Input
              id="conn-terminal-shell-path"
              value={form.terminalShellCustomPath}
              onChange={(e) =>
                setForm((current) => ({ ...current, terminalShellCustomPath: e.target.value }))
              }
              placeholder="C:\\Program Files\\PowerShell\\7\\pwsh.exe"
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
              placeholder="-NoLogo"
            />
          </div>
        </>
      )}
    </>
  )
}
