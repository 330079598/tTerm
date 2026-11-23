import rawFixture from "@fixtures/settings.reference.json"

import type { IntegrationStatus } from "@/components/SettingsDialog/useSystemIntegration"
import type { SecretStorageMode } from "@/lib/appConfig"
import { normalizeAppState, normalizeConfig, normalizeSecretStatus } from "@/lib/appConfigNormalize"
import type { ParityFixture } from "@/parity/fixtures"
import type { CustomTheme } from "@/types/theme"
import type { VerificationMethod } from "@/lib/userVerification"

function storageMode(value: string): SecretStorageMode {
  return value === "password" || value === "memory" ? value : "system"
}

function verificationMethod(value: string): VerificationMethod {
  return value === "system" || value === "masterPassword" ? value : "none"
}

const rawConfig = rawFixture.commands.load_config
const rawSecretStatus = rawFixture.commands.get_secret_backend_status

export const settingsReferenceConfig = normalizeConfig({
  ...rawConfig,
  ui_style: rawConfig.ui_style === "islands" ? "islands" : "classic",
  tab_width_mode: rawConfig.tab_width_mode === "standard" ? "standard" : "adaptive",
})
export const settingsReferenceAppState = normalizeAppState(rawFixture.commands.load_app_state)
export const settingsReferenceSecretStatus = normalizeSecretStatus({
  ...rawSecretStatus,
  storageMode: storageMode(rawSecretStatus.storageMode),
  verificationMethod: verificationMethod(rawSecretStatus.verificationMethod),
})

// `satisfies` keeps the JSON answer checked against the type the production
// command returns, so a renamed field breaks `pnpm typecheck`.
const customThemes = rawFixture.commands.load_custom_themes satisfies CustomTheme[]
const shellCommandStatus = rawFixture.commands.shell_command_status satisfies IntegrationStatus
const folderMenuStatus = rawFixture.commands.folder_menu_status satisfies IntegrationStatus
const systemFonts = rawFixture.commands.list_fonts satisfies string[]

/**
 * What the settings pages read through `ConfigProvider`, the theme store and
 * the general page. The values are synthetic and contain no real connection,
 * account or machine data.
 */
export const settingsReferenceFixture = {
  commands: {
    load_config: settingsReferenceConfig,
    load_app_state: settingsReferenceAppState,
    get_secret_backend_status: settingsReferenceSecretStatus,
    load_custom_themes: customThemes,
    save_window_background: rawFixture.commands.save_window_background,
    shell_command_status: shellCommandStatus,
    folder_menu_status: folderMenuStatus,
    list_fonts: systemFonts,
  },
} satisfies ParityFixture
