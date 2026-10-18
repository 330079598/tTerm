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
  /** A terminal tab, local or SSH, whose icon can be chosen. */
  iconTab?: Tab
  /** A local terminal tab: once named it can go back to an automatic name. */
  localTab?: Tab
}

export interface TabPanelsProps {
  activeTabId: string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  tabs: Tab[]
}
