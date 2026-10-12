import React from "react"

import type { AppearanceSettingsTabProps } from "@/components/SettingsDialog/AppearanceSettingsTab"
import { setWindowBlur } from "@/lib/themePreloader"
import { useSyncedState } from "@/hooks/useSyncedState"

type WindowBlurDraftOptions = Pick<
  AppearanceSettingsTabProps,
  | "windowBlur"
  | "windowBlurMaterial"
  | "windowBlurRadius"
  | "windowOpacityPercent"
  | "handleWindowBlurRadiusChange"
  | "handleWindowOpacityChange"
> & {
  /** False once the settings tab has unmounted. */
  mountedRef: React.RefObject<boolean>
}

/**
 * The blur radius and opacity sliders' drafts: previewed while dragged,
 * saved on release, and undone on close if never saved.
 */
export function useWindowBlurDraft({
  windowBlur,
  windowBlurMaterial,
  windowBlurRadius,
  windowOpacityPercent,
  handleWindowBlurRadiusChange,
  handleWindowOpacityChange,
  mountedRef,
}: WindowBlurDraftOptions) {
  const [blurRadiusDraft, setBlurRadiusDraft] = useSyncedState(windowBlurRadius)
  const [opacityDraft, setOpacityDraft] = useSyncedState(windowOpacityPercent)
  const savedWindowBlurRef = React.useRef({
    windowBlur,
    windowBlurMaterial,
    windowBlurRadius,
    windowOpacityPercent,
  })

  React.useEffect(() => {
    savedWindowBlurRef.current = {
      windowBlur,
      windowBlurMaterial,
      windowBlurRadius,
      windowOpacityPercent,
    }
  }, [windowBlur, windowBlurMaterial, windowBlurRadius, windowOpacityPercent])

  // Undo a slider preview that was never saved.
  React.useEffect(
    () => () => {
      const saved = savedWindowBlurRef.current
      setWindowBlur({
        enabled: saved.windowBlur,
        radius: saved.windowBlurRadius,
        material: saved.windowBlurMaterial,
        opacity: saved.windowOpacityPercent / 100,
      })
    },
    []
  )

  const previewWindowBlur = React.useCallback(
    (radius: number, opacityPercent: number) => {
      setBlurRadiusDraft(radius)
      setOpacityDraft(opacityPercent)
      setWindowBlur({
        enabled: true,
        radius,
        material: windowBlurMaterial,
        opacity: opacityPercent / 100,
      })
    },
    [setBlurRadiusDraft, setOpacityDraft, windowBlurMaterial]
  )

  const commitWindowBlurRadius = React.useCallback(async () => {
    if (blurRadiusDraft === windowBlurRadius) return
    const saved = await handleWindowBlurRadiusChange(blurRadiusDraft)
    if (!saved && mountedRef.current) {
      previewWindowBlur(windowBlurRadius, opacityDraft)
    }
  }, [
    blurRadiusDraft,
    handleWindowBlurRadiusChange,
    opacityDraft,
    previewWindowBlur,
    windowBlurRadius,
    mountedRef,
  ])

  const commitWindowOpacity = React.useCallback(async () => {
    if (opacityDraft === windowOpacityPercent) return
    const saved = await handleWindowOpacityChange(opacityDraft)
    if (!saved && mountedRef.current) {
      previewWindowBlur(blurRadiusDraft, windowOpacityPercent)
    }
  }, [
    blurRadiusDraft,
    handleWindowOpacityChange,
    opacityDraft,
    previewWindowBlur,
    windowOpacityPercent,
    mountedRef,
  ])

  return {
    blurRadiusDraft,
    opacityDraft,
    previewWindowBlur,
    commitWindowBlurRadius,
    commitWindowOpacity,
  }
}
