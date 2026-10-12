import { AppWindow } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { AppearanceSettingsTabProps } from "@/components/SettingsDialog/AppearanceSettingsTab"
import type { useWindowBlurDraft } from "@/components/SettingsDialog/useWindowBlurDraft"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { getDetectedPlatform } from "@/lib/uiPlatform"
import {
  WINDOW_BLUR_RADIUS_RANGE,
  WINDOW_OPACITY_PERCENT_RANGE,
  type WindowBlurMaterial,
} from "@/lib/appConfig"

type WindowBlurSectionProps = Pick<
  AppearanceSettingsTabProps,
  "windowBlur" | "windowBlurMaterial" | "handleWindowBlurChange" | "handleWindowBlurMaterialChange"
> &
  ReturnType<typeof useWindowBlurDraft>

/** The window's background blur: on or off, its material, radius and opacity. */
export function WindowBlurSection({
  windowBlur,
  windowBlurMaterial,
  handleWindowBlurChange,
  handleWindowBlurMaterialChange,
  blurRadiusDraft,
  opacityDraft,
  previewWindowBlur,
  commitWindowBlurRadius,
  commitWindowOpacity,
}: WindowBlurSectionProps) {
  const { t } = useTranslation()

  return (
    <SettingsSection
      icon={<AppWindow size={16} />}
      title={t("settings.window", { defaultValue: "Window" })}
    >
      <SettingsRow
        title={t("settings.windowBlur", { defaultValue: "Background blur" })}
        description={t("settings.windowBlurDesc", {
          defaultValue: "Blur what is behind the window and show it through the theme background.",
        })}
        action={
          <Switch
            aria-label={t("settings.windowBlur", { defaultValue: "Background blur" })}
            checked={windowBlur}
            onCheckedChange={(enabled) => void handleWindowBlurChange(enabled)}
          />
        }
      />
      {windowBlur && getDetectedPlatform() === "windows" && (
        <SettingsRow
          title={t("settings.windowBlurMaterial", { defaultValue: "Material" })}
          description={t("settings.windowBlurMaterialDesc", {
            defaultValue:
              "Acrylic blurs what is behind the window; on Windows 10 and early Windows 11 the window can lag while dragged. Mica tints with the desktop wallpaper instead and needs Windows 11.",
          })}
          action={
            <Select
              aria-label={t("settings.windowBlurMaterial", { defaultValue: "Material" })}
              value={windowBlurMaterial}
              onChange={(event) =>
                void handleWindowBlurMaterialChange(event.target.value as WindowBlurMaterial)
              }
              className="w-44"
            >
              <option value="acrylic">
                {t("settings.windowBlurMaterialAcrylic", { defaultValue: "Acrylic" })}
              </option>
              <option value="mica">
                {t("settings.windowBlurMaterialMica", { defaultValue: "Mica" })}
              </option>
            </Select>
          }
        />
      )}
      {windowBlur && (
        <>
          {getDetectedPlatform() === "macos" && (
            <SettingsRow
              title={t("settings.windowBlurRadius", { defaultValue: "Blur radius" })}
              description={t("settings.windowBlurRadiusDesc", {
                defaultValue: "Higher values blur the background more.",
              })}
            >
              <div className="flex w-full max-w-sm items-center gap-3">
                <input
                  type="range"
                  min={WINDOW_BLUR_RADIUS_RANGE.min}
                  max={WINDOW_BLUR_RADIUS_RANGE.max}
                  step={1}
                  value={blurRadiusDraft}
                  aria-label={t("settings.windowBlurRadius", { defaultValue: "Blur radius" })}
                  onChange={(event) => previewWindowBlur(Number(event.target.value), opacityDraft)}
                  onPointerUp={() => void commitWindowBlurRadius()}
                  onKeyUp={() => void commitWindowBlurRadius()}
                  onBlur={() => void commitWindowBlurRadius()}
                  className="accent-primary min-w-0 flex-1 cursor-pointer"
                />
                <output className="w-12 text-right text-sm font-medium" aria-live="polite">
                  {blurRadiusDraft}
                </output>
              </div>
            </SettingsRow>
          )}
          <SettingsRow
            title={t("settings.windowOpacity", { defaultValue: "Background opacity" })}
            description={t("settings.windowOpacityDesc", {
              defaultValue: "Lower values let more of the blurred background show through.",
            })}
          >
            <div className="flex w-full max-w-sm items-center gap-3">
              <input
                type="range"
                min={WINDOW_OPACITY_PERCENT_RANGE.min}
                max={WINDOW_OPACITY_PERCENT_RANGE.max}
                step={5}
                value={opacityDraft}
                aria-label={t("settings.windowOpacity", {
                  defaultValue: "Background opacity",
                })}
                aria-valuetext={`${opacityDraft}%`}
                onChange={(event) => previewWindowBlur(blurRadiusDraft, Number(event.target.value))}
                onPointerUp={() => void commitWindowOpacity()}
                onKeyUp={() => void commitWindowOpacity()}
                onBlur={() => void commitWindowOpacity()}
                className="accent-primary min-w-0 flex-1 cursor-pointer"
              />
              <output className="w-12 text-right text-sm font-medium" aria-live="polite">
                {opacityDraft}%
              </output>
            </div>
          </SettingsRow>
        </>
      )}
    </SettingsSection>
  )
}
