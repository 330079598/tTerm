import { SavedProfile } from "@/components/ProfilesPanel"
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
}

export interface EmptyStateProps {
  handleConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  handleNewTab: () => void
  onEditProfile: (profile: SavedProfile) => void
  profilesRefreshKey: number
}

export interface TabPanelsProps {
  activeTabId: string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  tabs: Tab[]
}
