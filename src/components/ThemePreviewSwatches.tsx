import React from "react"

import { cn } from "@/lib/utils"
import type { TerminalPalette } from "@/types/theme"

interface ThemePreviewSwatchesProps {
  className?: string
  compact?: boolean
  palette: TerminalPalette
}

const ANSI_KEYS: Array<keyof TerminalPalette> = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
]

const BRIGHT_ANSI_KEYS: Array<keyof TerminalPalette> = [
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
]

export const ThemePreviewSwatches: React.FC<ThemePreviewSwatchesProps> = ({
  className,
  compact = false,
  palette,
}) => {
  const dotSize = compact ? "size-2.5" : "size-3"

  return (
    <div className={cn("flex min-w-0 flex-col gap-1.5", className)}>
      <div className="flex items-center gap-1">
        <span
          className={cn("rounded-sm border border-white/10", compact ? "size-4" : "size-5")}
          style={{ background: palette.background }}
          title="Background"
        />
        <span
          className={cn("rounded-full border border-white/10", dotSize)}
          style={{ background: palette.foreground }}
          title="Foreground"
        />
        <span
          className={cn("rounded-full border border-white/10", dotSize)}
          style={{ background: palette.cursor }}
          title="Cursor"
        />
        <span
          className={cn("rounded-full border border-white/10", dotSize)}
          style={{ background: palette.selectionBackground }}
          title="Selection"
        />
      </div>
      <div className="flex flex-wrap gap-1">
        {ANSI_KEYS.map((key) => (
          <span
            key={key}
            className={cn("rounded-full border border-white/10", dotSize)}
            style={{ background: palette[key] }}
            title={key}
          />
        ))}
      </div>
      <div className="flex flex-wrap gap-1">
        {BRIGHT_ANSI_KEYS.map((key) => (
          <span
            key={key}
            className={cn("rounded-full border border-white/10", dotSize)}
            style={{ background: palette[key] }}
            title={key}
          />
        ))}
      </div>
    </div>
  )
}
