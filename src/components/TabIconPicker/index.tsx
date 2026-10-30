import { Ban } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TabIconGlyph } from "@/components/TabIcon"
import { Button } from "@/components/ui/button"
import { ColorSwatches } from "@/components/ui/color-swatches"
import { BRAND_LOGO_IDS, brandLogoColor, brandLogoName } from "@/lib/brandLogos"
import { DISTRO_IDS, distroName } from "@/lib/distro"
import {
  applyTabIconStyle,
  isDistroTabIcon,
  normalizeTabIconStyle,
  pickedLogoColor,
  TAB_ICON_COLORS,
  TAB_ICON_IDS,
  type ResolvedTabIcon,
  type TabIconChoice,
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
  // A logo brings its brand's color along, which the color row can then change.
  const chooseIcon = (icon: TabIconChoice) => {
    const color = pickedLogoColor(icon)
    choose({
      icon: icon === inherited.icon ? undefined : icon,
      ...(color !== undefined && { color: color === inherited.color ? undefined : color }),
    })
  }
  const chooseColor = (color: string) =>
    choose({ color: color === inherited.color ? undefined : color })

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <div className="text-muted-foreground text-xs">{t("tabIcons.icon")}</div>
        <div className="flex flex-wrap gap-1" role="radiogroup" aria-label={t("tabIcons.icon")}>
          {TAB_ICON_IDS.map((id) => (
            <IconOption
              key={id}
              icon={{ icon: id, color: shown.color }}
              name={t(`tabIcons.glyphs.${id}`)}
              checked={shown.icon === id}
              onSelect={chooseIcon}
            />
          ))}
        </div>
      </div>

      <div className="space-y-1.5">
        <div className="text-muted-foreground text-xs">{t("tabIcons.distros")}</div>
        <div className="flex flex-wrap gap-1" role="radiogroup" aria-label={t("tabIcons.distros")}>
          {DISTRO_IDS.map((id) => (
            <IconOption
              key={id}
              icon={{ icon: `distro:${id}`, color: "" }}
              name={distroName(id)}
              checked={shown.icon === `distro:${id}`}
              onSelect={chooseIcon}
            />
          ))}
        </div>
      </div>

      <div className="space-y-1.5">
        <div className="text-muted-foreground text-xs">{t("tabIcons.logos")}</div>
        <div className="flex flex-wrap gap-1" role="radiogroup" aria-label={t("tabIcons.logos")}>
          {BRAND_LOGO_IDS.map((id) => (
            <IconOption
              key={id}
              icon={{ icon: `logo:${id}`, color: brandLogoColor(id) }}
              name={brandLogoName(id)}
              checked={shown.icon === `logo:${id}`}
              onSelect={chooseIcon}
            />
          ))}
        </div>
      </div>

      {/* A distribution's logo keeps its own colors. */}
      <div className={cn("space-y-1.5", isDistroTabIcon(shown.icon) && "hidden")}>
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

/** One icon to pick, shown as it would look on a tab. */
function IconOption({
  icon,
  name,
  checked,
  onSelect,
}: {
  icon: ResolvedTabIcon & { icon: TabIconChoice }
  name: string
  checked: boolean
  onSelect: (icon: TabIconChoice) => void
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      aria-label={name}
      title={name}
      className={cn(
        "hover:bg-muted flex h-7 w-7 items-center justify-center rounded-md border",
        checked ? "border-primary bg-muted" : "border-transparent"
      )}
      onClick={() => onSelect(icon.icon)}
    >
      <TabIconGlyph icon={icon} size={14} />
    </button>
  )
}
