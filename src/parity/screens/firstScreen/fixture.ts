import rawProfiles from "@fixtures/first-screen.profiles.json"
import rawTerminal from "@fixtures/first-screen.terminal.json"

import { paneLeaf, type TabGroup } from "@/lib/tabGroups"
import type { ParityFixture } from "@/parity/fixtures"
import type { ProfileGroup, SavedProfile, Tab } from "@/types/tab"

/** The synthetic saved hosts both captures show, with no account or secret. */
export const firstScreenProfiles = rawProfiles.commands.list_profiles.map((profile) => ({
  ...profile,
  session_log: profile.session_log as SavedProfile["session_log"],
  proxy: profile.proxy as SavedProfile["proxy"],
})) satisfies SavedProfile[]
export const firstScreenGroups = rawProfiles.commands.list_profile_groups satisfies ProfileGroup[]

/**
 * The one local tab both captures show. Its title is the fallback title the
 * native client gives a `--exec` tab, so the selected tab and its label match.
 */
export const firstScreenTabs: Tab[] = [
  {
    id: "first-screen-local",
    title: "Terminal",
    type: "terminal",
    isActive: true,
    hasConnected: true,
    autoTitle: true,
    shellName: "auto",
    connection: {
      type: "terminal",
      cwd: "~/work/demo",
      terminalShell: "auto",
    },
  },
]

export const firstScreenActiveTabId = "first-screen-local"

/** Each visible tab is its own group, as the workspace stores it. */
export const firstScreenGroupsForTabs = firstScreenTabs.map((tab) => ({
  id: `group-${tab.id}`,
  root: paneLeaf(tab.id),
  activeTabId: tab.id,
})) satisfies TabGroup[]

/** The host picker answers the same fixture the native instance seeds. */
export const firstScreenProfilesFixture = {
  commands: {
    list_profiles: firstScreenProfiles,
    list_profile_groups: firstScreenGroups,
  },
} satisfies ParityFixture

/**
 * The deterministic terminal output both renderers receive. The last line is
 * the waiting prompt, so it carries no final newline: the cursor stays on it
 * instead of moving to an empty trailing line.
 */
export const firstScreenTerminalOutput = rawTerminal.lines.join("\r\n")
