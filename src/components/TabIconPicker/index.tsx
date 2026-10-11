import { Ban } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TAB_ICON_GLYPHS } from "@/components/TabIconPicker/glyphs"
import { Button } from "@/components/ui/button"
import {
  applyTabIconStyle,
  normalizeTabIconStyle,
  TAB_ICON_COLORS,
  TAB_ICON_IDS,
  type ResolvedTabIcon,
  type TabIconStyle,
} from "@/lib/tabIcons"
import { cn } from "@/lib/utils"

interface TabIconPickerProps {
  /** The choice being edited; a missing icon or color keeps `inherited`'s. */
  value: TabIconStyle | undefined
  /** What shows while nothing is chosen. */
  inherited: ResolvedTabIcon
  onChange: (value: TabIconStyle | undefined) => void
  /** Names the button that drops the choice, shown while there is one. */
  resetLabel: string
}

const SWATCH = "flex h-6 w-6 items-center justify-center rounded-full border-2 transition-colors"

/**
 * Picks an icon and a color. Picking what is inherited drops that part of
 * the choice, so it follows the inherited one again when that changes.
 */
export function TabIconPicker({ value, inherited, onChange, resetLabel }: TabIconPickerProps) {
  const { t } = useTranslation()
  const shown = applyTabIconStyle(inherited, value)
  const choose = (patch: TabIconStyle) => onChange(normalizeTabIconStyle({ ...value, ...patch }))
  const chooseIcon = (icon: TabIconStyle["icon"]) =>
    choose({ icon: icon === inherited.icon ? undefined : icon })
  const chooseColor = (color: string) =>
    choose({ color: color === inherited.color ? undefined : color })
  const isCustomColor =
    shown.color !== "" && !(TAB_ICON_COLORS as readonly string[]).includes(shown.color)

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <div className="text-muted-foreground text-xs">{t("tabIcons.icon")}</div>
        <div className="flex flex-wrap gap-1" role="radiogroup" aria-label={t("tabIcons.icon")}>
          {TAB_ICON_IDS.map((id) => {
            const Glyph = TAB_ICON_GLYPHS[id]
            const name = t(`tabIcons.glyphs.${id}`)
            return (
              <button
                key={id}
                type="button"
                role="radio"
                aria-checked={shown.icon === id}
                aria-label={name}
                title={name}
                className={cn(
                  "hover:bg-muted flex h-7 w-7 items-center justify-center rounded-md border",
                  shown.icon === id ? "border-primary bg-muted" : "border-transparent"
                )}
                onClick={() => chooseIcon(id)}
              >
                <Glyph
                  size={14}
                  style={shown.color ? { color: shown.color } : undefined}
                  aria-hidden="true"
                />
              </button>
            )
          })}
        </div>
      </div>

      <div className="space-y-1.5">
        <div className="text-muted-foreground text-xs">{t("tabIcons.color")}</div>
        <div
          className="flex flex-wrap items-center gap-1.5"
          role="radiogroup"
          aria-label={t("tabIcons.color")}
        >
          <button
            type="button"
            role="radio"
            aria-checked={shown.color === ""}
            aria-label={t("tabIcons.noColor")}
            title={t("tabIcons.noColor")}
            className={cn(SWATCH, shown.color === "" ? "border-primary" : "border-border")}
            onClick={() => chooseColor("")}
          >
            <Ban size={12} className="text-muted-foreground" aria-hidden="true" />
          </button>
          {TAB_ICON_COLORS.map((color) => (
            <button
              key={color}
              type="button"
              role="radio"
              aria-checked={shown.color === color}
              aria-label={color}
              title={color}
              className={cn(
                SWATCH,
                shown.color === color ? "border-primary" : "border-transparent"
              )}
              onClick={() => chooseColor(color)}
            >
              <span className="h-4 w-4 rounded-full" style={{ backgroundColor: color }} />
            </button>
          ))}
          <label
            title={t("tabIcons.customColor")}
            className={cn(
              SWATCH,
              "relative cursor-pointer overflow-hidden",
              isCustomColor ? "border-primary" : "border-border"
            )}
          >
            <span
              className="h-4 w-4 rounded-full"
              style={{
                background: isCustomColor
                  ? shown.color
                  : "conic-gradient(#e5534b, #d4a72c, #4caf7a, #3b9ae1, #a371f7, #e5534b)",
              }}
            />
            <input
              type="color"
              aria-label={t("tabIcons.customColor")}
              className="absolute inset-0 cursor-pointer opacity-0"
              value={isCustomColor ? shown.color : "#8b949e"}
              onChange={(event) => chooseColor(event.target.value.toLowerCase())}
            />
          </label>
        </div>
      </div>

      {value && (
        <Button type="button" variant="ghost" size="sm" onClick={() => onChange(undefined)}>
          {resetLabel}
        </Button>
      )}
    </div>
  )
}
