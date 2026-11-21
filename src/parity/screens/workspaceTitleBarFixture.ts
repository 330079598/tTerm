import rawFixture from "@fixtures/workspace.titlebar.default.json"

import type { SecretStorageMode } from "@/lib/appConfig"
import { normalizeAppState, normalizeConfig, normalizeSecretStatus } from "@/lib/appConfigNormalize"
import { paneLeaf, type TabGroup } from "@/lib/tabGroups"
import type { ParityFixture } from "@/parity/fixtures"
import type { VerificationMethod } from "@/lib/userVerification"
import type { Tab } from "@/types/tab"

/**
 * Two local Windows shells. The paths and names are made up: the reference
 * screen must never carry a real account or project from a developer machine.
 */
export const workspaceTitleBarTabs: Tab[] = [
  {
    id: "local-pwsh",
    title: "app",
    type: "terminal",
    isActive: true,
    hasConnected: true,
    autoTitle: true,
    shellName: "pwsh",
    connection: {
      type: "terminal",
      cwd: "C:\\Users\\demo\\source\\app",
      terminalShell: "pwsh",
    },
  },
  {
    id: "local-cmd",
    title: "cmd",
    type: "terminal",
    isActive: false,
    hasConnected: true,
    shellName: "cmd",
    connection: {
      type: "terminal",
      cwd: "C:\\Users\\demo",
      terminalShell: "cmd",
    },
  },
]

export const workspaceTitleBarActiveTabId = "local-pwsh"

/** Each tab is a group of its own, with its single pane in front. */
export const workspaceTitleBarGroups = workspaceTitleBarTabs.map((tab) => ({
  id: `group-${tab.id}`,
  root: paneLeaf(tab.id),
  activeTabId: tab.id,
})) satisfies TabGroup[]

const rawConfig = rawFixture.commands.load_config
const rawSecretStatus = rawFixture.commands.get_secret_backend_status

function storageMode(value: string): SecretStorageMode {
  return value === "password" || value === "memory" ? value : "system"
}

function verificationMethod(value: string): VerificationMethod {
  return value === "system" || value === "masterPassword" ? value : "none"
}

export const workspaceTitleBarConfig = normalizeConfig({
  ...rawConfig,
  ui_style: rawConfig.ui_style === "islands" ? "islands" : "classic",
  tab_width_mode: rawConfig.tab_width_mode === "standard" ? "standard" : "adaptive",
})
export const workspaceTitleBarAppState = normalizeAppState(rawFixture.commands.load_app_state)
export const workspaceTitleBarSecretStatus = normalizeSecretStatus({
  ...rawSecretStatus,
  storageMode: storageMode(rawSecretStatus.storageMode),
  verificationMethod: verificationMethod(rawSecretStatus.verificationMethod),
})

/** The commands the title bar reads through `ConfigProvider`, with production normalizers. */
export const workspaceTitleBarFixture = {
  commands: {
    load_config: workspaceTitleBarConfig,
    load_app_state: workspaceTitleBarAppState,
    get_secret_backend_status: workspaceTitleBarSecretStatus,
  },
} satisfies ParityFixture
