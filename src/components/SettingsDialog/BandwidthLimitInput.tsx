import React, { useState } from "react"
import { useTranslation } from "react-i18next"

import { Input } from "@/components/ui/input"
import { Select } from "@/components/ui/select"
import { MAX_BANDWIDTH_LIMIT_KIB } from "@/contexts/ConfigContext"
import { isImeKeyEvent } from "@/lib/ime"

export type BandwidthUnit = "KB" | "MB"

/** The unit a stored limit reads best in: whole MB/s when it is one. */
export function preferredUnit(kib: number): BandwidthUnit {
  return kib > 0 && kib % 1024 === 0 ? "MB" : "KB"
}

export function formatLimit(kib: number, unit: BandwidthUnit): string {
  if (unit === "KB") return String(kib)
  return String(Math.round((kib / 1024) * 100) / 100)
}

/** Parses the typed value in `unit` to KiB/s; null when it is not a number. */
export function parseLimit(text: string, unit: BandwidthUnit): number | null {
  const value = Number(text.trim())
  if (text.trim() === "" || !Number.isFinite(value)) return null
  const kib = Math.round(unit === "MB" ? value * 1024 : value)
  return Math.min(Math.max(kib, 0), MAX_BANDWIDTH_LIMIT_KIB)
}

interface BandwidthLimitInputProps {
  "aria-label": string
  /** KiB/s; 0 is unlimited. */
  value: number
  onChange: (kib: number) => Promise<void> | void
}

/**
 * Number + unit input for a bandwidth limit. The value is committed on blur
 * or Enter rather than per keystroke: the limit applies to running transfers
 * immediately, and typing "1024" must not throttle them to 1 KB/s on the way.
 */
export const BandwidthLimitInput: React.FC<BandwidthLimitInputProps> = ({
  "aria-label": ariaLabel,
  value,
  onChange,
}) => {
  const { t } = useTranslation()
  const [unit, setUnit] = useState<BandwidthUnit>(() => preferredUnit(value))
  const [draft, setDraft] = useState(() => formatLimit(value, preferredUnit(value)))

  // Follow the stored value when it changes from elsewhere (or is normalised),
  // adjusting during render rather than in an effect.
  const [syncedValue, setSyncedValue] = useState(value)
  if (value !== syncedValue) {
    const nextUnit = preferredUnit(value)
    setSyncedValue(value)
    setUnit(nextUnit)
    setDraft(formatLimit(value, nextUnit))
  }

  const commit = () => {
    // An untouched (possibly rounded) display of the stored value must not
    // re-save it: 100 KB/s shown as "0.1" MB/s would become 102 KB/s.
    if (draft === formatLimit(value, unit)) return
    const kib = parseLimit(draft, unit)
    if (kib === null) {
      setDraft(formatLimit(value, unit))
      return
    }
    if (kib !== value) void onChange(kib)
    else setDraft(formatLimit(value, unit))
  }

  return (
    <div className="flex items-center gap-2">
      <Input
        type="number"
        min={0}
        step={unit === "MB" ? 0.1 : 1}
        inputMode="decimal"
        aria-label={ariaLabel}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
            event.preventDefault()
            commit()
          }
        }}
        className="w-24"
      />
      <Select
        aria-label={t("settings.sftpBandwidthUnit", { defaultValue: "Unit" })}
        value={unit}
        onChange={(event) => {
          // Switching units keeps the rate and only changes how it reads.
          const nextUnit = event.target.value as BandwidthUnit
          const kib = parseLimit(draft, unit) ?? value
          setUnit(nextUnit)
          setDraft(formatLimit(kib, nextUnit))
        }}
        className="w-24"
      >
        <option value="KB">KB/s</option>
        <option value="MB">MB/s</option>
      </Select>
    </div>
  )
}
