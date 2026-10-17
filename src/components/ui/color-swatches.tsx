import type React from "react"

import { cn } from "@/lib/utils"

const SWATCH = "flex h-6 w-6 items-center justify-center rounded-full border-2 transition-colors"

interface ColorSwatchesProps {
  /** The chosen color as `#rrggbb`, or "" for the empty choice. */
  value: string
  /** The preset colors, as lowercase `#rrggbb`. */
  colors: readonly string[]
  onChange: (color: string) => void
  /** Names the group for screen readers. */
  label: string
  /** The swatch that picks "": its name and what it shows inside. */
  empty: { label: string; content: React.ReactNode }
  /** Names the swatch that opens the system color picker. */
  customLabel: string
}

/**
 * A row of round color swatches: an empty choice, the presets, and one that
 * opens the system color picker for any other color.
 */
export function ColorSwatches({
  value,
  colors,
  onChange,
  label,
  empty,
  customLabel,
}: ColorSwatchesProps) {
  const isCustom = value !== "" && !colors.includes(value)

  return (
    <div className="flex flex-wrap items-center gap-1.5" role="radiogroup" aria-label={label}>
      <button
        type="button"
        role="radio"
        aria-checked={value === ""}
        aria-label={empty.label}
        title={empty.label}
        className={cn(SWATCH, value === "" ? "border-primary" : "border-border")}
        onClick={() => onChange("")}
      >
        {empty.content}
      </button>
      {colors.map((color) => (
        <button
          key={color}
          type="button"
          role="radio"
          aria-checked={value === color}
          aria-label={color}
          title={color}
          className={cn(SWATCH, value === color ? "border-primary" : "border-transparent")}
          onClick={() => onChange(color)}
        >
          <span className="h-4 w-4 rounded-full" style={{ backgroundColor: color }} />
        </button>
      ))}
      <label
        title={customLabel}
        className={cn(
          SWATCH,
          "relative cursor-pointer overflow-hidden",
          isCustom ? "border-primary" : "border-border"
        )}
      >
        <span
          className="h-4 w-4 rounded-full"
          style={{
            background: isCustom
              ? value
              : "conic-gradient(#e5534b, #d4a72c, #4caf7a, #3b9ae1, #a371f7, #e5534b)",
          }}
        />
        <input
          type="color"
          aria-label={customLabel}
          className="absolute inset-0 cursor-pointer opacity-0"
          value={isCustom ? value : "#8b949e"}
          onChange={(event) => onChange(event.target.value.toLowerCase())}
        />
      </label>
    </div>
  )
}
