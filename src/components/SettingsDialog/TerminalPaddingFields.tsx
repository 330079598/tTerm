import { useTranslation } from "react-i18next"

import type { FontSettingsTabProps } from "@/components/SettingsDialog/FontSettingsTab"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

type TerminalPaddingFieldsProps = Pick<
  FontSettingsTabProps,
  | "terminalPaddingLeftPx"
  | "terminalPaddingRightPx"
  | "terminalPaddingBottomPx"
  | "setTerminalPaddingLeftPx"
  | "setTerminalPaddingRightPx"
  | "setTerminalPaddingBottomPx"
>

/** The space between the terminal's text and the left, right and bottom edges. */
export function TerminalPaddingFields({
  terminalPaddingLeftPx,
  terminalPaddingRightPx,
  terminalPaddingBottomPx,
  setTerminalPaddingLeftPx,
  setTerminalPaddingRightPx,
  setTerminalPaddingBottomPx,
}: TerminalPaddingFieldsProps) {
  const { t } = useTranslation()
  const terminalPaddingFields = [
    {
      id: "terminal-padding-left",
      label: t("fontSettings.terminalPaddingLeft", { defaultValue: "Left" }),
      value: terminalPaddingLeftPx,
      setter: setTerminalPaddingLeftPx,
    },
    {
      id: "terminal-padding-right",
      label: t("fontSettings.terminalPaddingRight", { defaultValue: "Right" }),
      value: terminalPaddingRightPx,
      setter: setTerminalPaddingRightPx,
    },
    {
      id: "terminal-padding-bottom",
      label: t("fontSettings.terminalPaddingBottom", { defaultValue: "Bottom" }),
      value: terminalPaddingBottomPx,
      setter: setTerminalPaddingBottomPx,
    },
  ]

  return (
    <div>
      <Label className="mb-2 block">
        {t("fontSettings.terminalPadding", { defaultValue: "Terminal Padding" })}
      </Label>
      <div className="grid gap-3 sm:grid-cols-3">
        {terminalPaddingFields.map((field) => (
          <div key={field.id} className="space-y-1.5">
            <Label htmlFor={field.id} className="text-muted-foreground text-xs">
              {field.label}
            </Label>
            <Input
              id={field.id}
              type="number"
              min={0}
              max={80}
              value={field.value}
              onChange={(e) => {
                const value = parseInt(e.target.value)
                if (!isNaN(value) && value >= 0 && value <= 80) field.setter(value)
              }}
              className="h-8"
            />
          </div>
        ))}
      </div>
      <p className="text-muted-foreground mt-2 text-xs">
        {t("fontSettings.terminalPaddingDesc", {
          defaultValue:
            "Adjust left, right, and bottom spacing in pixels. Bottom spacing lifts the last terminal line away from the window edge.",
        })}
      </p>
    </div>
  )
}
