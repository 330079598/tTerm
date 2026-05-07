import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useToast } from "@/hooks/use-toast"

export interface SettingsDialogProps {
  onClose: () => void
  defaultTab?: string
}

export interface SettingsPanelProps {
  defaultTab?: string
  className?: string
}

export type ConfigState = ReturnType<typeof useConfig>["config"]
export type SecretStatusState = ReturnType<typeof useConfig>["secretStatus"]
export type SaveConfig = ReturnType<typeof useConfig>["saveConfig"]
export type UpdateLanguage = ReturnType<typeof useConfig>["updateLanguage"]
export type RefreshSecretStatus = ReturnType<typeof useConfig>["refreshSecretStatus"]
export type SetSecretVaultEnabled = ReturnType<typeof useConfig>["setSecretVaultEnabled"]
export type UnlockSecretVault = ReturnType<typeof useConfig>["unlockSecretVault"]
export type LockSecretVault = ReturnType<typeof useConfig>["lockSecretVault"]

export type ThemeContextState = ReturnType<typeof useTheme>
export type ToastState = ReturnType<typeof useToast>["toast"]

export const FONT_SIZE_OPTIONS = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24]

export const CURSOR_STYLE_OPTIONS = [
  { value: "bar", labelKey: "fontSettings.cursorStyles.bar" },
  { value: "block", labelKey: "fontSettings.cursorStyles.block" },
  { value: "underline", labelKey: "fontSettings.cursorStyles.underline" },
] as const

export const languages = [
  { code: "en", label: "English", nativeLabel: "English" },
  { code: "zh", label: "中文", nativeLabel: "Chinese" },
]
