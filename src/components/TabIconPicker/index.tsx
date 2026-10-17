import { Ban } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TAB_ICON_GLYPHS } from "@/components/TabIconPicker/glyphs"
import { Button } from "@/components/ui/button"
import { ColorSwatches } from "@/components/ui/color-swatches"
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
        <ColorSwatches
          value={shown.color}
          colors={TAB_ICON_COLORS}
          onChange={chooseColor}
          label={t("tabIcons.color")}
          empty={{
            label: t("tabIcons.noColor"),
            content: <Ban size={12} className="text-muted-foreground" aria-hidden="true" />,
          }}
          customLabel={t("tabIcons.customColor")}
        />
      </div>

      {value && (
        <Button type="button" variant="ghost" size="sm" onClick={() => onChange(undefined)}>
          {resetLabel}
        </Button>
      )}
    </div>
  )
}
