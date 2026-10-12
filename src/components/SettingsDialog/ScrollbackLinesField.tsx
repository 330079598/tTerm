import { useTranslation } from "react-i18next"

import type { FontSettingsTabProps } from "@/components/SettingsDialog/FontSettingsTab"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  DEFAULT_SCROLLBACK_LINES,
  MAX_EXPLICIT_SCROLLBACK_LINES,
  UNLIMITED_SCROLLBACK_BUFFER,
  UNLIMITED_SCROLLBACK_SENTINEL,
  isUnlimitedScrollback,
} from "@/lib/scrollback"
import { cn } from "@/lib/utils"

const SCROLLBACK_PRESETS = [1000, 5000, DEFAULT_SCROLLBACK_LINES, 50000, 100000] as const

type ScrollbackLinesFieldProps = Pick<
  FontSettingsTabProps,
  "scrollbackLines" | "setScrollbackLines"
>

/** How many lines of history a terminal keeps, typed or picked from presets. */
export function ScrollbackLinesField({
  scrollbackLines,
  setScrollbackLines,
}: ScrollbackLinesFieldProps) {
  const { t } = useTranslation()

  return (
    <div>
      <Label className="mb-2 block">
        {t("fontSettings.scrollbackLines", { defaultValue: "Scrollback Lines" })}
      </Label>
      <div className="space-y-2">
        <div className="flex gap-2">
          <Input
            type="number"
            min={0}
            max={MAX_EXPLICIT_SCROLLBACK_LINES}
            value={scrollbackLines}
            onChange={(e) => {
              const value = parseInt(e.target.value, 10)
              if (
                !isNaN(value) &&
                value >= UNLIMITED_SCROLLBACK_SENTINEL &&
                value <= MAX_EXPLICIT_SCROLLBACK_LINES
              ) {
                setScrollbackLines(value)
              }
            }}
            className="flex-1"
            placeholder={t("fontSettings.scrollbackLinesPlaceholder", {
              defaultValue: "Default 10000, or pick a preset",
            })}
          />
        </div>
        <p className="text-muted-foreground text-xs">
          {t("fontSettings.scrollbackLinesDesc", {
            defaultValue: "Default: 10,000 lines. Unlimited history can use significant memory.",
          })}
        </p>
        {isUnlimitedScrollback(scrollbackLines) && (
          <p className="text-xs text-amber-600 dark:text-amber-500">
            {t("fontSettings.scrollbackUnlimitedWarning", {
              max: UNLIMITED_SCROLLBACK_BUFFER.toLocaleString(),
              defaultValue:
                "Unlimited is not truly unlimited: history keeps growing up to {{max}} lines and is only freed when you clear the screen. Memory per line grows with the window width; at 120 columns, 1 million lines take about 1.5 GB. All tabs share one interface process, and if their total memory goes past the limit the system allows it (about 16 GB on machines with plenty of RAM, less on smaller ones), the window may crash and every tab loses its scrollback. To keep output long term, turn on Terminal session logging.",
            })}
          </p>
        )}
        <div className="flex flex-wrap gap-1.5">
          <Button
            type="button"
            variant={isUnlimitedScrollback(scrollbackLines) ? "default" : "outline"}
            size="xs"
            onClick={() => setScrollbackLines(UNLIMITED_SCROLLBACK_SENTINEL)}
            className={cn(
              "min-w-[3.5rem]",
              !isUnlimitedScrollback(scrollbackLines) && "text-muted-foreground"
            )}
          >
            {t("fontSettings.unlimited", { defaultValue: "Unlimited" })}
          </Button>
          {SCROLLBACK_PRESETS.map((lines) => (
            <Button
              key={lines}
              type="button"
              variant={scrollbackLines === lines ? "default" : "outline"}
              size="xs"
              onClick={() => setScrollbackLines(lines)}
              className={cn("min-w-[3.5rem]", scrollbackLines !== lines && "text-muted-foreground")}
            >
              {lines >= 1000 ? `${lines / 1000}k` : lines}
            </Button>
          ))}
        </div>
      </div>
    </div>
  )
}
