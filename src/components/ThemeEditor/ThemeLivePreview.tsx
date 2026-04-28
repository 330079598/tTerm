import React from "react"
import { useTranslation } from "react-i18next"

import type { TerminalPalette, ThemeColors } from "@/types/theme"

import {
  colorToHex,
  contrastRatio,
  normalizeColorPreview,
  resolveCssColor,
} from "@/components/ThemeEditor/colorUtils"

interface ThemeLivePreviewProps {
  colors: ThemeColors
  terminal: TerminalPalette
}

export const ThemeLivePreview: React.FC<ThemeLivePreviewProps> = ({ colors, terminal }) => {
  const { t } = useTranslation()
  const contrastChecks = [
    {
      label: t("themeEditor.preview.contrast.text"),
      ratio: contrastRatio(
        colorToHex(colors.background) ?? "",
        colorToHex(colors.foreground) ?? ""
      ),
    },
    {
      label: t("themeEditor.preview.contrast.primary"),
      ratio: contrastRatio(
        colorToHex(colors.primary) ?? "",
        colorToHex(colors.primaryForeground) ?? ""
      ),
    },
    {
      label: t("themeEditor.preview.contrast.card"),
      ratio: contrastRatio(colorToHex(colors.card) ?? "", colorToHex(colors.cardForeground) ?? ""),
    },
  ]

  return (
    <aside className="xl:sticky xl:top-0 xl:self-start">
      <div className="border-border bg-card overflow-hidden rounded-2xl border shadow-sm">
        <div className="border-border border-b px-4 py-3">
          <p className="text-sm font-semibold">{t("themeEditor.preview.title")}</p>
          <p className="text-muted-foreground text-xs">{t("themeEditor.preview.description")}</p>
        </div>

        <div
          className="space-y-4 p-4"
          style={{
            background: resolveCssColor(colors.background),
            color: resolveCssColor(colors.foreground),
          }}
        >
          <div
            className="overflow-hidden rounded-xl border"
            style={{
              borderColor: resolveCssColor(colors.border),
              background: resolveCssColor(colors.card),
              color: resolveCssColor(colors.cardForeground),
            }}
          >
            <div
              className="flex items-center justify-between px-3 py-2"
              style={{ background: resolveCssColor(colors.titlebar) }}
            >
              <div className="flex gap-1.5">
                <span className="size-2.5 rounded-full bg-red-400" />
                <span className="size-2.5 rounded-full bg-yellow-400" />
                <span className="size-2.5 rounded-full bg-green-400" />
              </div>
              <span className="text-[11px] font-medium">
                {t("themeEditor.preview.windowTitle")}
              </span>
            </div>
            <div
              className="flex gap-1 border-y px-2 py-2"
              style={{
                borderColor: resolveCssColor(colors.border),
                background: resolveCssColor(colors.tabBackground),
              }}
            >
              <span
                className="rounded-md px-2 py-1 text-xs"
                style={{
                  background: resolveCssColor(colors.tabActive),
                  color: resolveCssColor(colors.foreground),
                }}
              >
                {t("themeEditor.preview.localTab")}
              </span>
              <span
                className="rounded-md px-2 py-1 text-xs"
                style={{
                  background: resolveCssColor(colors.tabHover),
                  color: resolveCssColor(colors.mutedForeground),
                }}
              >
                {t("themeEditor.preview.sshTab")}
              </span>
            </div>
            <div className="space-y-3 p-3">
              <div
                className="rounded-lg border p-3"
                style={{
                  borderColor: resolveCssColor(colors.border),
                  background: resolveCssColor(colors.background),
                }}
              >
                <p className="text-sm font-semibold">
                  {t("themeEditor.preview.connectionSettings")}
                </p>
                <p
                  className="mt-1 text-xs"
                  style={{ color: resolveCssColor(colors.mutedForeground) }}
                >
                  {t("themeEditor.preview.connectionDescription")}
                </p>
                <div className="mt-3 flex gap-2">
                  <button
                    type="button"
                    className="rounded-md px-3 py-1.5 text-xs font-medium"
                    style={{
                      background: resolveCssColor(colors.primary),
                      color: resolveCssColor(colors.primaryForeground),
                    }}
                  >
                    {t("connection.connect")}
                  </button>
                  <button
                    type="button"
                    className="rounded-md border px-3 py-1.5 text-xs"
                    style={{
                      borderColor: resolveCssColor(colors.border),
                      background: resolveCssColor(colors.secondary),
                      color: resolveCssColor(colors.secondaryForeground),
                    }}
                  >
                    {t("common.cancel")}
                  </button>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-2 text-center text-[11px]">
                <PreviewBadge
                  label={t("themeEditor.preview.success")}
                  background={colors.success}
                  foreground={colors.successForeground}
                />
                <PreviewBadge
                  label={t("themeEditor.preview.warning")}
                  background={colors.warning}
                  foreground={colors.warningForeground}
                />
                <PreviewBadge
                  label={t("themeEditor.preview.danger")}
                  background={colors.destructive}
                  foreground={colors.destructiveForeground}
                />
              </div>
            </div>
          </div>

          <div
            className="overflow-hidden rounded-xl border font-mono text-xs"
            style={{
              borderColor: resolveCssColor(colors.border),
              background: normalizeColorPreview(terminal.background),
              color: normalizeColorPreview(terminal.foreground),
            }}
          >
            <div
              className="border-b px-3 py-2"
              style={{
                borderColor: resolveCssColor(colors.border),
                color: normalizeColorPreview(terminal.cursor),
              }}
            >
              ~/theme-lab
            </div>
            <div className="space-y-1 p-3">
              <p>
                <span style={{ color: normalizeColorPreview(terminal.green) }}>stone@tterm</span>:
                <span style={{ color: normalizeColorPreview(terminal.blue) }}>~/Code</span>$ pnpm
                dev
              </p>
              <p>
                <span style={{ color: normalizeColorPreview(terminal.yellow) }}>vite</span>{" "}
                {t("themeEditor.preview.readyIn")}{" "}
                <span style={{ color: normalizeColorPreview(terminal.cyan) }}>312ms</span>
              </p>
              <p style={{ color: normalizeColorPreview(terminal.magenta) }}>
                {t("themeEditor.preview.updated")}
              </p>
            </div>
          </div>

          <div className="grid grid-cols-3 gap-2">
            {contrastChecks.map((check) => (
              <div
                key={check.label}
                className="rounded-lg border px-2 py-2 text-center"
                style={{
                  borderColor: resolveCssColor(colors.border),
                  background: resolveCssColor(colors.card),
                }}
              >
                <p
                  className="text-[10px]"
                  style={{ color: resolveCssColor(colors.mutedForeground) }}
                >
                  {check.label}
                </p>
                <p className="text-xs font-semibold">
                  {check.ratio ? check.ratio.toFixed(1) : "--"}:1
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </aside>
  )
}

interface PreviewBadgeProps {
  label: string
  background: string
  foreground: string
}

const PreviewBadge: React.FC<PreviewBadgeProps> = ({ label, background, foreground }) => {
  return (
    <div
      className="rounded-md px-2 py-2 font-medium"
      style={{ background: resolveCssColor(background), color: resolveCssColor(foreground) }}
    >
      {label}
    </div>
  )
}
