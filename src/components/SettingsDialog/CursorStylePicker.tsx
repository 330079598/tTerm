import React from "react"
import { useTranslation } from "react-i18next"

import type { AppConfig } from "@/contexts/ConfigContext"
import { cn } from "@/lib/utils"
import { CURSOR_STYLE_OPTIONS } from "@/components/SettingsDialog/types"

type CursorStyle = AppConfig["cursor_style"]

const SAMPLE_PROMPT = "guest@tterm:~$"

function CursorSampleGlyph({ style }: { style: CursorStyle }) {
  if (style === "bar") {
    return <span className="inline-block h-[1.1em] w-[2px] bg-current align-[-0.18em]" />
  }

  if (style === "underline") {
    return (
      <span className="inline-flex h-[1.1em] w-[0.7em] items-end align-[-0.18em]">
        <span className="block h-[2px] w-full bg-current" />
      </span>
    )
  }

  return <span className="inline-block h-[1.1em] w-[0.7em] bg-current align-[-0.18em]" />
}

interface CursorStylePickerProps {
  value: CursorStyle
  onChange: (value: CursorStyle) => void
}

export const CursorStylePicker: React.FC<CursorStylePickerProps> = ({ value, onChange }) => {
  const { t } = useTranslation()

  return (
    <div className="grid gap-2 sm:grid-cols-3">
      {CURSOR_STYLE_OPTIONS.map((option) => {
        const selected = option.value === value

        return (
          <button
            key={option.value}
            type="button"
            onClick={() => onChange(option.value)}
            aria-label={t(option.labelKey)}
            title={t(option.labelKey)}
            className={cn(
              "border-border bg-secondary/40 hover:bg-accent hover:border-accent-foreground/20 text-foreground rounded-lg border px-3 py-3 text-left transition-colors",
              selected && "border-primary bg-accent ring-primary/20 ring-2"
            )}
          >
            <span className="block overflow-hidden rounded bg-black/80 px-3 py-2 font-mono text-sm text-slate-100">
              <span>{SAMPLE_PROMPT}</span>
              <CursorSampleGlyph style={option.value} />
            </span>
          </button>
        )
      })}
    </div>
  )
}
