import React from "react"
import { Ban } from "lucide-react"
import { useTranslation } from "react-i18next"

import { HIGHLIGHT_PALETTE_COLORS } from "@/lib/keywordHighlight/config"
import { type HighlightPalette, resolveHighlightColor } from "@/lib/keywordHighlight/match"
import { cn } from "@/lib/utils"

const SWATCH =
  "flex size-6 cursor-pointer items-center justify-center rounded-md border-2 transition-colors"

interface HighlightColorPickerProps {
  /** A palette color name, `#rrggbb`, or null for none. */
  value: string | null
  onChange: (color: string | null) => void
  /** The terminal colors the palette swatches show. */
  palette: HighlightPalette
  label: string
}

/**
 * Picks a highlight color: none, one of the terminal theme's sixteen colors
 * (which follow the theme), or any other from the system color picker.
 */
export const HighlightColorPicker: React.FC<HighlightColorPickerProps> = ({
  value,
  onChange,
  palette,
  label,
}) => {
  const { t } = useTranslation()
  const isCustom = value !== null && !HIGHLIGHT_PALETTE_COLORS.some((name) => name === value)
  const customColor = isCustom ? (resolveHighlightColor(value, palette) ?? "#8b949e") : "#8b949e"

  return (
    <div className="flex flex-wrap items-start gap-1.5" role="radiogroup" aria-label={label}>
      <button
        type="button"
        role="radio"
        aria-checked={value === null}
        aria-label={t("keywordHighlight.colorNone")}
        title={t("keywordHighlight.colorNone")}
        className={cn(SWATCH, value === null ? "border-primary" : "border-border")}
        onClick={() => onChange(null)}
      >
        <Ban size={12} className="text-muted-foreground" aria-hidden="true" />
      </button>
      <div className="grid grid-cols-8 gap-1.5">
        {HIGHLIGHT_PALETTE_COLORS.map((name) => {
          const colorLabel = t(`keywordHighlight.colors.${name}`)
          return (
            <button
              key={name}
              type="button"
              role="radio"
              aria-checked={value === name}
              aria-label={colorLabel}
              title={colorLabel}
              className={cn(SWATCH, value === name ? "border-primary" : "border-transparent")}
              onClick={() => onChange(name)}
            >
              <span
                className="size-4 rounded-sm"
                style={{ backgroundColor: resolveHighlightColor(name, palette) }}
              />
            </button>
          )
        })}
      </div>
      <label
        title={t("keywordHighlight.colorCustom")}
        className={cn(
          SWATCH,
          "relative overflow-hidden",
          isCustom ? "border-primary" : "border-border"
        )}
      >
        <span
          className="size-4 rounded-sm"
          style={{
            background: isCustom
              ? customColor
              : "conic-gradient(#e5534b, #d4a72c, #4caf7a, #3b9ae1, #a371f7, #e5534b)",
          }}
        />
        <input
          type="color"
          aria-label={t("keywordHighlight.colorCustom")}
          className="absolute inset-0 cursor-pointer opacity-0"
          value={customColor}
          onChange={(event) => onChange(event.target.value.toLowerCase())}
        />
      </label>
    </div>
  )
}
