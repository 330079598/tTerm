import { useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"

import {
  DEFAULT_TERMINAL_FONT_SIZE,
  TERMINAL_FONT_SIZE_RANGE,
  useConfig,
} from "@/contexts/ConfigContext"
import { useKeymap } from "@/contexts/KeymapContext"
import { toast } from "@/hooks/use-toast"
import { useStableRef } from "@/hooks/useStableRef"
import { toErrorMessage } from "@/lib/utils"

export function clampTerminalFontSize(size: number): number {
  return Math.min(
    Math.max(Math.round(size), TERMINAL_FONT_SIZE_RANGE.min),
    TERMINAL_FONT_SIZE_RANGE.max
  )
}

/** Ctrl+= / Ctrl+- / Ctrl+0: change the terminal font size for every tab and save it. */
export function useTerminalFontZoom() {
  const { config, saveConfig } = useConfig()
  const { registerHandler } = useKeymap()
  const { t } = useTranslation()
  const fontSizeRef = useStableRef(config.font_size)
  // The size the last press asked for while its save is still running, so
  // presses faster than a save add up instead of starting from the old size.
  const targetRef = useRef<number | null>(null)
  const pendingSavesRef = useRef(0)

  // Settings or sync changed the size: start from it again.
  useEffect(() => {
    if (pendingSavesRef.current === 0) targetRef.current = null
  }, [config.font_size])

  useEffect(() => {
    const zoomTo = (next: number) => {
      if (next === (targetRef.current ?? fontSizeRef.current)) return
      targetRef.current = next
      pendingSavesRef.current += 1
      saveConfig({ font_size: next })
        .then(() => {
          pendingSavesRef.current -= 1
        })
        .catch((error) => {
          pendingSavesRef.current -= 1
          // A failed save leaves the config as it was.
          if (pendingSavesRef.current === 0) targetRef.current = null
          console.error("Failed to save terminal font size:", error)
          toast({
            title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
            description: toErrorMessage(error),
            variant: "destructive",
          })
        })
    }
    const zoomBy = (step: number) => () =>
      zoomTo(clampTerminalFontSize((targetRef.current ?? fontSizeRef.current) + step))

    const unregister = [
      registerHandler("terminal.zoomIn", zoomBy(1)),
      registerHandler("terminal.zoomOut", zoomBy(-1)),
      registerHandler("terminal.zoomReset", () => zoomTo(DEFAULT_TERMINAL_FONT_SIZE)),
    ]
    return () => unregister.forEach((dispose) => dispose())
  }, [fontSizeRef, registerHandler, saveConfig, t])
}
