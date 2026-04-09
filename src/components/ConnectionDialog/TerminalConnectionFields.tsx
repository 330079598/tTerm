import React from "react"
import { useTranslation } from "react-i18next"

import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { TerminalShellType } from "@/types/tab"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface TerminalConnectionFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

export const TerminalConnectionFields: React.FC<TerminalConnectionFieldsProps> = ({
  form,
  setForm,
}) => {
  const { t } = useTranslation()

  return (
    <>
      <div>
        <Label htmlFor="conn-terminal-shell" className="mb-1.5 block">
          {t("connection.terminalShell")}
        </Label>
        <Select
          id="conn-terminal-shell"
          value={form.terminalShell}
          onChange={(e) =>
            setForm((current) => ({
              ...current,
              terminalShell: e.target.value as TerminalShellType,
            }))
          }
        >
          <option value="auto">{t("connection.terminalShellOptions.auto")}</option>
          <option value="cmd">{t("connection.terminalShellOptions.cmd")}</option>
          <option value="powershell">{t("connection.terminalShellOptions.powershell")}</option>
          <option value="pwsh">{t("connection.terminalShellOptions.pwsh")}</option>
          <option value="custom">{t("connection.terminalShellOptions.custom")}</option>
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
