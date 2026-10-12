import type { TFunction } from "i18next"
import type { RefObject } from "react"

import type { TabPanelsHandle } from "@/components/TTermApp/TabPanels"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import type { UseTabsReturn } from "@/hooks/useTabs"
import type { Tab } from "@/types/tab"

/**
 * What the app's hooks share: the tabs and the actions on them, refs that
 * always hold the latest tabs, the workspace, and the confirmation dialog.
 */
export interface AppShell extends UseTabsReturn {
  t: TFunction
  tabsRef: RefObject<Tab[]>
  activeTabIdRef: RefObject<string | null>
  workspaceRef: RefObject<TabPanelsHandle | null>
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  cleanupConnection: (tabId: string) => void
}
