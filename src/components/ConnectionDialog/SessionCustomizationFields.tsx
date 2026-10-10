import React from "react"
import { useTranslation } from "react-i18next"

import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import { useTheme } from "@/contexts/ThemeContext"
import type { TerminalPalette } from "@/types/theme"

import { TerminalThemePicker } from "@/components/ConnectionDialog/TerminalThemePicker"
import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface SessionCustomizationFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

const PREVIEW_COLORS = ["red", "green", "yellow", "blue", "magenta", "cyan"] as const

const PalettePreview: React.FC<{ palette: TerminalPalette }> = ({ palette }) => (
  <div
    aria-hidden="true"
    className="mt-2 flex items-center justify-between gap-3 rounded-md border px-3 py-2 font-mono text-xs"
    style={{ background: palette.background, color: palette.foreground }}
  >
    <span className="truncate">
      <span style={{ color: palette.green }}>user@prod</span>:
      <span style={{ color: palette.blue }}>~</span>$ uptime
    </span>
    <span className="flex shrink-0 gap-1">
      {PREVIEW_COLORS.map((color) => (
        <span
          key={color}
          className="size-2.5 rounded-full"
          style={{ background: palette[color] }}
        />
      ))}
    </span>
  </div>
)

export const SessionCustomizationFields: React.FC<SessionCustomizationFieldsProps> = ({
  form,
  setForm,
}) => {
  const { t } = useTranslation()
  const { getTheme } = useTheme()
  const selectedTheme = form.terminalTheme ? getTheme(form.terminalTheme) : undefined

  return (
    <div className="space-y-3 rounded-md border px-3 py-2">
      <div>
        <Label htmlFor="conn-terminal-theme" className="mb-1.5 block">
          {t("connection.terminalTheme")}
        </Label>
        <TerminalThemePicker
          id="conn-terminal-theme"
          value={form.terminalTheme}
          onChange={(terminalTheme) => setForm((current) => ({ ...current, terminalTheme }))}
        />
        {selectedTheme && <PalettePreview palette={selectedTheme.terminal} />}
        <p className="text-muted-foreground mt-1 text-xs">{t("connection.terminalThemeDesc")}</p>
      </div>

      <div>
        <Label htmlFor="conn-login-script" className="mb-1.5 block">
          {t("connection.loginScript")}
        </Label>
        <Textarea
          id="conn-login-script"
          value={form.loginScript}
          onChange={(e) => setForm((current) => ({ ...current, loginScript: e.target.value }))}
          placeholder={t("connection.loginScriptPlaceholder")}
          className="min-h-16 font-mono"
          rows={3}
        />
        <p className="text-muted-foreground mt-1 text-xs">{t("connection.loginScriptDesc")}</p>
      </div>

      <div>
        <Label htmlFor="conn-session-log" className="mb-1.5 block">
          {t("connection.sessionLog")}
        </Label>
        <Select
          id="conn-session-log"
          value={form.sessionLog}
          onChange={(e) => {
            const value = e.target.value
            setForm((current) => ({
              ...current,
              sessionLog: value === "always" || value === "never" ? value : "default",
            }))
          }}
        >
          {(["default", "always", "never"] as const).map((option) => (
            <option key={option} value={option}>
              {t(`connection.sessionLogOptions.${option}`)}
            </option>
          ))}
        </Select>
        <p className="text-muted-foreground mt-1 text-xs">{t("connection.sessionLogDesc")}</p>
      </div>

      <div className="flex items-start gap-2">
        <Checkbox
          id="conn-shell-integration"
          checked={form.shellIntegration}
          onCheckedChange={(checked) =>
            setForm((current) => ({ ...current, shellIntegration: checked }))
          }
        />
        <div className="space-y-0.5">
          <Label htmlFor="conn-shell-integration" className="text-sm font-normal">
            {t("connection.shellIntegration")}
          </Label>
          <p className="text-muted-foreground text-xs">{t("connection.shellIntegrationDesc")}</p>
        </div>
      </div>
    </div>
  )
}
