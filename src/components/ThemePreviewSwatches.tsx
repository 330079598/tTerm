import React from "react"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
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
        <Tooltip>
          <TooltipTrigger asChild>
            <span
              className={cn("rounded-sm border border-white/10", compact ? "size-4" : "size-5")}
              style={{ background: palette.background }}
            />
          </TooltipTrigger>
          <TooltipContent>Background</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <span
              className={cn("rounded-full border border-white/10", dotSize)}
              style={{ background: palette.foreground }}
            />
          </TooltipTrigger>
          <TooltipContent>Foreground</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <span
              className={cn("rounded-full border border-white/10", dotSize)}
              style={{ background: palette.cursor }}
            />
          </TooltipTrigger>
          <TooltipContent>Cursor</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <span
              className={cn("rounded-full border border-white/10", dotSize)}
              style={{ background: palette.selectionBackground }}
            />
          </TooltipTrigger>
          <TooltipContent>Selection</TooltipContent>
        </Tooltip>
      </div>
      <div className="flex flex-wrap gap-1">
        {ANSI_KEYS.map((key) => (
          <Tooltip key={key}>
            <TooltipTrigger asChild>
              <span
                className={cn("rounded-full border border-white/10", dotSize)}
                style={{ background: palette[key] }}
              />
            </TooltipTrigger>
            <TooltipContent>{key}</TooltipContent>
          </Tooltip>
        ))}
      </div>
      <div className="flex flex-wrap gap-1">
        {BRIGHT_ANSI_KEYS.map((key) => (
          <Tooltip key={key}>
            <TooltipTrigger asChild>
              <span
                className={cn("rounded-full border border-white/10", dotSize)}
                style={{ background: palette[key] }}
              />
            </TooltipTrigger>
            <TooltipContent>{key}</TooltipContent>
          </Tooltip>
        ))}
      </div>
    </div>
  )
}
