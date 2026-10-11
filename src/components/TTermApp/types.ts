import { Tab, TabContextMenuAction } from "@/types/tab"

export interface ContextMenuState {
  visible: boolean
  x: number
  y: number
  tab: Tab | null
  actions: TabContextMenuAction[]
}

export interface RenameDialogState {
  isOpen: boolean
  tabId: string | null
  currentName: string
  /** A local terminal tab: its icon can be chosen, and once named it can go back to an automatic name. */
  localTab?: Tab
}

export interface TabPanelsProps {
  activeTabId: string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  tabs: Tab[]
}
