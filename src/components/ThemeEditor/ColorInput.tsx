import React, { useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

import {
  clamp,
  colorToHex,
  hexToHslParts,
  hexToHslToken,
  hslToRgb,
  normalizeColorPreview,
  readableTextHex,
  rgbToHex,
} from "@/components/ThemeEditor/colorUtils"
import { TERMINAL_PLACEHOLDER } from "@/components/ThemeEditor/themeEditorConstants"
import type { HslParts } from "@/components/ThemeEditor/types"

interface ColorInputProps {
  label: string
  value: string
  onChange: (value: string) => void
  colorMode?: "hsl-token" | "css"
  placeholder?: string
  suggestions?: string[]
  token?: string
  description?: string
  isOpen?: boolean
  onOpenChange?: (open: boolean) => void
}

export const ColorInput: React.FC<ColorInputProps> = ({
  label,
  value,
  onChange,
  colorMode = "hsl-token",
  placeholder = "0 0% 0%",
  suggestions = [],
  token,
  description,
  isOpen = false,
  onOpenChange,
}) => {
  const { t } = useTranslation()
  const rootRef = useRef<HTMLDivElement | null>(null)
  const [draftValue, setDraftValue] = useState(value)
  const [pickerHsl, setPickerHsl] = useState<HslParts>(() => {
    const initialHex = colorToHex(value) ?? "#000000"
    return hexToHslParts(initialHex) ?? { h: 0, s: 0, l: 0 }
  })
  const currentValue = isOpen ? draftValue : value
  const pickerValue = colorToHex(currentValue) ?? "#000000"
  const derivedHslParts = hexToHslParts(pickerValue) ?? { h: 0, s: 0, l: 0 }
  const hslParts = isOpen ? pickerHsl : derivedHslParts
  const previewText = readableTextHex(pickerValue)

  useEffect(() => {
    if (!isOpen) return

    const closeWithoutSaving = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return
      onOpenChange?.(false)
    }

    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onOpenChange?.(false)
    }

    document.addEventListener("pointerdown", closeWithoutSaving)
    document.addEventListener("keydown", closeOnEscape)

    return () => {
      document.removeEventListener("pointerdown", closeWithoutSaving)
      document.removeEventListener("keydown", closeOnEscape)
    }
  }, [isOpen, onOpenChange])

  const handleOpenChange = (open: boolean) => {
    if (open) {
      setDraftValue(value)
      const nextHex = colorToHex(value) ?? "#000000"
      setPickerHsl(hexToHslParts(nextHex) ?? { h: 0, s: 0, l: 0 })
    }

    onOpenChange?.(open)
  }

  const handleDraftChange = (nextValue: string) => {
    if (isOpen) {
      setDraftValue(nextValue)
      const nextHex = colorToHex(nextValue)
      const nextHsl = nextHex ? hexToHslParts(nextHex) : null
      if (nextHsl) setPickerHsl(nextHsl)
      return
    }

    onChange(nextValue)
  }

  const handleDone = () => {
    onChange(draftValue)
    onOpenChange?.(false)
  }

  const applyHex = (nextHex: string) => {
    const nextHsl = hexToHslParts(nextHex)
    if (nextHsl) setPickerHsl(nextHsl)

    if (colorMode === "hsl-token") {
      const hslToken = hexToHslToken(nextHex)
      if (hslToken) setDraftValue(hslToken)
      return
    }

    setDraftValue(nextHex)
  }

  const applyHsl = (updates: Partial<HslParts>) => {
    const next = {
      h: clamp(updates.h ?? hslParts.h, 0, 360),
      s: clamp(updates.s ?? hslParts.s, 0, 100),
      l: clamp(updates.l ?? hslParts.l, 0, 100),
    }
    setPickerHsl(next)

    if (colorMode === "hsl-token") {
      setDraftValue(`${Math.round(next.h)} ${Math.round(next.s)}% ${Math.round(next.l)}%`)
      return
    }

    const rgb = hslToRgb(next.h, next.s, next.l)
    setDraftValue(rgbToHex(rgb.r, rgb.g, rgb.b))
  }

  return (
    <div
      ref={rootRef}
      className="hover:border-border/80 hover:bg-muted/20 relative rounded-lg border border-transparent p-2 transition-colors"
    >
      <div className="flex items-center gap-3">
        <button
          type="button"
          aria-label={t("themeEditor.colorPicker.open", { label })}
          onClick={() => handleOpenChange(!isOpen)}
          className="border-border ring-offset-background focus-visible:ring-ring relative size-10 shrink-0 overflow-hidden rounded-lg border shadow-sm transition-transform hover:scale-105 focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
          style={{ backgroundColor: normalizeColorPreview(currentValue) }}
        >
          <span className="absolute inset-x-0 bottom-0 h-2 bg-black/15" />
        </button>
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center justify-between gap-2">
            <div className="min-w-0">
              <Label className="text-xs font-medium">{label}</Label>
              {token && <p className="text-muted-foreground font-mono text-[10px]">{token}</p>}
            </div>
            <span className="text-muted-foreground font-mono text-[10px] uppercase">
              {pickerValue}
            </span>
          </div>
          {description && <p className="text-muted-foreground mb-2 text-xs">{description}</p>}
          <Input
            value={currentValue}
            onFocus={() => handleOpenChange(true)}
            onChange={(e) => handleDraftChange(e.target.value)}
            placeholder={placeholder}
            className="h-8 font-mono text-sm"
          />
        </div>
      </div>

      {isOpen && (
        <div className="bg-popover text-popover-foreground border-border absolute top-full right-0 z-50 mt-2 w-[min(20rem,calc(100vw-3rem))] rounded-xl border p-3 shadow-2xl">
          <SaturationLightnessPicker
            hsl={hslParts}
            previewHex={pickerValue}
            previewText={previewText}
            onChange={(updates) => applyHsl(updates)}
          />

          <div className="mt-3 space-y-3">
            <ColorSlider
              label={t("themeEditor.colorPicker.hue")}
              value={hslParts.h}
              min={0}
              max={360}
              background="linear-gradient(90deg, #ef4444, #f59e0b, #eab308, #22c55e, #06b6d4, #3b82f6, #a855f7, #ef4444)"
              onChange={(next) => applyHsl({ h: next })}
            />
          </div>

          <div className="mt-3 grid grid-cols-[1fr_auto] gap-2">
            <Input
              value={pickerValue}
              onChange={(e) => applyHex(e.target.value)}
              className="h-8 font-mono text-sm"
            />
            <input
              aria-label={t("themeEditor.colorPicker.systemPicker", { label })}
              type="color"
              value={pickerValue}
              onChange={(e) => applyHex(e.target.value)}
              className="border-input bg-background h-8 w-10 cursor-pointer rounded-md border p-1"
            />
          </div>

          {suggestions.length > 0 && (
            <div className="mt-3">
              <p className="text-muted-foreground mb-2 text-[11px] font-medium">
                {t("themeEditor.colorPicker.themeColors")}
              </p>
              <div className="grid grid-cols-10 gap-1.5">
                {suggestions.slice(0, 20).map((suggestion) => (
                  <button
                    key={suggestion}
                    type="button"
                    aria-label={t("themeEditor.colorPicker.useColor", { color: suggestion })}
                    onClick={() => applyHex(suggestion)}
                    className="border-border size-5 rounded border shadow-sm transition-transform hover:scale-110"
                    style={{ backgroundColor: suggestion }}
                  />
                ))}
              </div>
            </div>
          )}

          <div className="mt-3 flex items-center justify-between gap-2 border-t pt-3">
            <div className="text-muted-foreground text-[11px]">
              {t("themeEditor.colorPicker.textOnColor")}{" "}
              <span className="font-mono">{previewText}</span>
            </div>
            <Button type="button" variant="outline" size="sm" onClick={handleDone}>
              {t("common.done")}
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}

interface SaturationLightnessPickerProps {
  hsl: HslParts
  previewHex: string
  previewText: string
  onChange: (updates: Partial<Pick<HslParts, "s" | "l">>) => void
}

const SaturationLightnessPicker: React.FC<SaturationLightnessPickerProps> = ({
  hsl,
  previewHex,
  previewText,
  onChange,
}) => {
  const { t } = useTranslation()
  const pickerRef = useRef<HTMLDivElement | null>(null)
  const [isDragging, setIsDragging] = useState(false)

  const updateFromPointer = (clientX: number, clientY: number) => {
    const rect = pickerRef.current?.getBoundingClientRect()
    if (!rect) return

    const x = clamp((clientX - rect.left) / rect.width, 0, 1)
    const y = clamp((clientY - rect.top) / rect.height, 0, 1)
    onChange({ s: Math.round(x * 100), l: Math.round((1 - y) * 100) })
  }

  const handlePointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    setIsDragging(true)
    updateFromPointer(event.clientX, event.clientY)
  }

  const handlePointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging) return
    updateFromPointer(event.clientX, event.clientY)
  }

  const stopDragging = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
    setIsDragging(false)
  }

  return (
    <div>
      <div
        ref={pickerRef}
        role="slider"
        tabIndex={0}
        aria-label={t("themeEditor.colorPicker.saturationLightness")}
        aria-valuetext={t("themeEditor.colorPicker.saturationLightnessValue", {
          saturation: Math.round(hsl.s),
          lightness: Math.round(hsl.l),
        })}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={stopDragging}
        onPointerCancel={stopDragging}
        className="border-border relative h-32 touch-none overflow-hidden rounded-lg border shadow-inner"
        style={{ backgroundColor: `hsl(${hsl.h} 100% 50%)` }}
      >
        <div className="absolute inset-0 bg-gradient-to-r from-white to-transparent" />
        <div className="absolute inset-0 bg-gradient-to-t from-black to-transparent" />
        <div
          className="absolute size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.65),0_4px_12px_rgba(0,0,0,0.35)]"
          style={{ left: `${hsl.s}%`, top: `${100 - hsl.l}%`, backgroundColor: previewHex }}
        />
        <div
          className="absolute bottom-3 left-3 rounded-md px-2 py-1 text-xs font-medium shadow"
          style={{ backgroundColor: previewHex, color: previewText }}
        >
          {previewHex}
        </div>
      </div>
      <p className="text-muted-foreground mt-2 text-[11px]">
        {t("themeEditor.colorPicker.saturationLightnessHelp")}
      </p>
    </div>
  )
}

interface ColorSliderProps {
  label: string
  value: number
  min: number
  max: number
  background: string
  onChange: (value: number) => void
}

const ColorSlider: React.FC<ColorSliderProps> = ({
  label,
  value,
  min,
  max,
  background,
  onChange,
}) => {
  const sliderRef = useRef<HTMLDivElement | null>(null)

  const range = max - min
  const valueRatio = range === 0 ? 0 : clamp((value - min) / range, 0, 1)

  const updateFromPointer = (clientX: number) => {
    const rect = sliderRef.current?.getBoundingClientRect()
    if (!rect) return

    const ratio = clamp((clientX - rect.left) / rect.width, 0, 1)
    onChange(Math.round(min + ratio * range))
  }

  const handlePointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    updateFromPointer(event.clientX)
  }

  const handlePointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return
    updateFromPointer(event.clientX)
  }

  const stopDragging = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 10 : 1
    const roundedValue = Math.round(value)
    let nextValue = roundedValue

    if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
      nextValue = roundedValue - step
    } else if (event.key === "ArrowRight" || event.key === "ArrowUp") {
      nextValue = roundedValue + step
    } else if (event.key === "Home") {
      nextValue = min
    } else if (event.key === "End") {
      nextValue = max
    } else {
      return
    }

    event.preventDefault()
    onChange(clamp(nextValue, min, max))
  }

  return (
    <div className="block space-y-1">
      <div className="flex items-center justify-between text-[11px]">
        <span id={`${label}-slider-label`} className="text-muted-foreground font-medium">
          {label}
        </span>
        <span className="font-mono">{Math.round(value)}</span>
      </div>
      <div
        ref={sliderRef}
        role="slider"
        tabIndex={0}
        aria-labelledby={`${label}-slider-label`}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={Math.round(value)}
        onKeyDown={handleKeyDown}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={stopDragging}
        onPointerCancel={stopDragging}
        className="border-border focus-visible:ring-ring relative h-3 w-full cursor-pointer touch-none rounded-full border focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
        style={{ background }}
      >
        <span
          className="bg-primary absolute top-1/2 size-5 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white shadow-[0_1px_6px_rgba(0,0,0,0.35)]"
          style={{ left: `${valueRatio * 100}%` }}
        />
      </div>
    </div>
  )
}

export const TerminalColorInput: React.FC<Omit<ColorInputProps, "colorMode" | "placeholder">> = ({
  label,
  value,
  onChange,
  suggestions,
  isOpen,
  onOpenChange,
}) => {
  return (
    <ColorInput
      label={label}
      value={value}
      onChange={onChange}
      colorMode="css"
      placeholder={TERMINAL_PLACEHOLDER}
      suggestions={suggestions}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
    />
  )
}
