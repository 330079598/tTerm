/** Central registry of every user-bindable action in the app. */

export type KeymapActionGroup = "workspace" | "tabs" | "terminal" | "sftp" | "zmodem" | "editor"

export type KeymapActionId =
  // workspace
  | "workspace.newTab"
  | "workspace.splitRight"
  | "workspace.splitBelow"
  | "workspace.focusNextPane"
  | "workspace.focusPrevPane"
  | "workspace.focusPaneLeft"
  | "workspace.focusPaneRight"
  | "workspace.focusPaneUp"
  | "workspace.focusPaneDown"
  | "workspace.restoreSingleGroup"
  | "commandLibrary.open"
  | "broadcast.togglePanel"
  // tabs
  | "tabs.next"
  | "tabs.prev"
  | "tabs.switchToNth"
  | "tabs.renameActive"
  | "tabs.closeActive"
  // terminal
  | "terminal.find"
  | "terminal.clear"
  | "terminal.reset"
  | "terminal.saveSelection"
  | "terminal.fillSavedPassword"
  | "terminal.zoomIn"
  | "terminal.zoomOut"
  | "terminal.zoomReset"
  | "terminal.previousCommand"
  | "terminal.nextCommand"
  | "terminal.copyLastCommandOutput"
  // sftp
  | "sftp.toggle"
  | "sftp.selectAll"
  | "sftp.focusPath"
  | "sftp.pasteUpload"
  | "sftp.back"
  | "sftp.forward"
  | "sftp.up"
  // zmodem
  | "zmodem.sendFiles"
  | "zmodem.receiveFiles"
  // editor
  | "editor.save"

export interface KeymapActionDefinition {
  id: KeymapActionId
  group: KeymapActionGroup
  /**
   * When false the action is skipped while the focus sits on an editable
   * surface (input/textarea/contenteditable — the xterm helper textarea
   * included), so chords like Ctrl+A keep reaching the remote shell.
   */
  allowInEditable: boolean
}

export const KEYMAP_ACTIONS: KeymapActionDefinition[] = [
  { id: "workspace.newTab", group: "workspace", allowInEditable: true },
  { id: "workspace.splitRight", group: "workspace", allowInEditable: true },
  { id: "workspace.splitBelow", group: "workspace", allowInEditable: true },
  { id: "workspace.focusNextPane", group: "workspace", allowInEditable: true },
  { id: "workspace.focusPrevPane", group: "workspace", allowInEditable: true },
  { id: "workspace.focusPaneLeft", group: "workspace", allowInEditable: true },
  { id: "workspace.focusPaneRight", group: "workspace", allowInEditable: true },
  { id: "workspace.focusPaneUp", group: "workspace", allowInEditable: true },
  { id: "workspace.focusPaneDown", group: "workspace", allowInEditable: true },
  { id: "workspace.restoreSingleGroup", group: "workspace", allowInEditable: true },
  { id: "commandLibrary.open", group: "workspace", allowInEditable: true },
  { id: "broadcast.togglePanel", group: "workspace", allowInEditable: true },
  { id: "tabs.next", group: "tabs", allowInEditable: true },
  { id: "tabs.prev", group: "tabs", allowInEditable: true },
  { id: "tabs.switchToNth", group: "tabs", allowInEditable: true },
  { id: "tabs.renameActive", group: "tabs", allowInEditable: true },
  { id: "tabs.closeActive", group: "tabs", allowInEditable: true },
  { id: "terminal.find", group: "terminal", allowInEditable: true },
  { id: "terminal.clear", group: "terminal", allowInEditable: true },
  { id: "terminal.reset", group: "terminal", allowInEditable: true },
  { id: "terminal.saveSelection", group: "terminal", allowInEditable: true },
  { id: "terminal.fillSavedPassword", group: "terminal", allowInEditable: true },
  { id: "terminal.zoomIn", group: "terminal", allowInEditable: true },
  { id: "terminal.zoomOut", group: "terminal", allowInEditable: true },
  { id: "terminal.zoomReset", group: "terminal", allowInEditable: true },
  { id: "terminal.previousCommand", group: "terminal", allowInEditable: true },
  { id: "terminal.nextCommand", group: "terminal", allowInEditable: true },
  { id: "terminal.copyLastCommandOutput", group: "terminal", allowInEditable: true },
  { id: "sftp.toggle", group: "sftp", allowInEditable: true },
  { id: "sftp.selectAll", group: "sftp", allowInEditable: false },
  { id: "sftp.focusPath", group: "sftp", allowInEditable: false },
  { id: "sftp.pasteUpload", group: "sftp", allowInEditable: false },
  { id: "sftp.back", group: "sftp", allowInEditable: false },
  { id: "sftp.forward", group: "sftp", allowInEditable: false },
  { id: "sftp.up", group: "sftp", allowInEditable: false },
  { id: "zmodem.sendFiles", group: "zmodem", allowInEditable: true },
  { id: "zmodem.receiveFiles", group: "zmodem", allowInEditable: true },
  { id: "editor.save", group: "editor", allowInEditable: true },
]

const KEYMAP_ACTION_MAP = new Map(KEYMAP_ACTIONS.map((action) => [action.id, action]))

export const KEYMAP_ACTION_IDS: KeymapActionId[] = KEYMAP_ACTIONS.map((action) => action.id)

export function getKeymapAction(id: KeymapActionId): KeymapActionDefinition | undefined {
  return KEYMAP_ACTION_MAP.get(id)
}

/** Ordered groups for the settings UI. */
export const KEYMAP_ACTION_GROUPS: KeymapActionGroup[] = [
  "workspace",
  "tabs",
  "terminal",
  "sftp",
  "zmodem",
  "editor",
]

export function getActionsByGroup(group: KeymapActionGroup): KeymapActionDefinition[] {
  return KEYMAP_ACTIONS.filter((action) => action.group === group)
}
