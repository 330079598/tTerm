import { useEffect } from "react"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { useKeymap } from "@/contexts/KeymapContext"
import type { useTerminalContextMenu } from "@/components/TerminalTab/useTerminalContextMenu"
import type { TerminalTabProps } from "@/components/TerminalTab/types"

type UseTerminalShortcutsOptions = Pick<TerminalTabProps, "onSaveCommand"> &
  Pick<
    TerminalTabRefs,
    | "isShortcutTargetRef"
    | "savedPasswordPromptActionsRef"
    | "termRef"
    | "commandMarksRef"
    | "containerRef"
    | "connectionRef"
  > &
  Pick<ReturnType<typeof useKeymap>, "registerHandler"> & {
    openSearch: () => void
    handleToggleSftpDrawer: () => void
  } & Pick<
    ReturnType<typeof useTerminalContextMenu>,
    "clearTerminalHistory" | "resetTerminal" | "armZmodemManualTrigger" | "copyLastCommandOutput"
  >

/** The terminal's keyboard shortcuts; each serves the focused pane only. */
export function useTerminalShortcuts({
  onSaveCommand,
  isShortcutTargetRef,
  savedPasswordPromptActionsRef,
  termRef,
  commandMarksRef,
  containerRef,
  connectionRef,
  registerHandler,
  openSearch,
  handleToggleSftpDrawer,
  clearTerminalHistory,
  resetTerminal,
  armZmodemManualTrigger,
  copyLastCommandOutput,
}: UseTerminalShortcutsOptions) {
  useEffect(() => {
    const unregisterFind = registerHandler("terminal.find", () => {
      if (!isShortcutTargetRef.current) return false
      openSearch()
    })
    const unregisterClear = registerHandler("terminal.clear", () => {
      if (!isShortcutTargetRef.current) return false
      clearTerminalHistory()
    })
    const unregisterReset = registerHandler("terminal.reset", () => {
      if (!isShortcutTargetRef.current) return false
      void resetTerminal()
    })
    const unregisterToggleSftp = registerHandler("sftp.toggle", () => {
      if (!isShortcutTargetRef.current) return false
      handleToggleSftpDrawer()
    })
    const unregisterZmodemSend = registerHandler("zmodem.sendFiles", () => {
      if (!isShortcutTargetRef.current) return false
      void armZmodemManualTrigger("send")
    })
    const unregisterZmodemReceive = registerHandler("zmodem.receiveFiles", () => {
      if (!isShortcutTargetRef.current) return false
      void armZmodemManualTrigger("receive")
    })
    const unregisterFillSavedPassword = registerHandler("terminal.fillSavedPassword", () => {
      if (!isShortcutTargetRef.current) return false
      const actions = savedPasswordPromptActionsRef.current
      if (!actions) return false
      if (actions.fill()) {
        termRef.current?.focus()
        return
      }
      // Declining would let xterm send the combo as Enter, submitting a
      // half-typed password; at a password prompt the chord is a no-op.
      if (!actions.atPasswordPrompt()) return false
    })
    const unregisterPreviousCommand = registerHandler("terminal.previousCommand", () => {
      if (!isShortcutTargetRef.current) return false
      return commandMarksRef.current?.scrollToPreviousPrompt() ?? false
    })
    const unregisterNextCommand = registerHandler("terminal.nextCommand", () => {
      if (!isShortcutTargetRef.current) return false
      return commandMarksRef.current?.scrollToNextPrompt() ?? false
    })
    const unregisterCopyCommandOutput = registerHandler("terminal.copyLastCommandOutput", () => {
      if (!isShortcutTargetRef.current || !commandMarksRef.current?.lastOutputRange()) return false
      void copyLastCommandOutput()
    })
    const unregisterSaveSelection = registerHandler("terminal.saveSelection", () => {
      if (!isShortcutTargetRef.current) return false
      if (!containerRef.current?.contains(document.activeElement)) return false
      const selection = termRef.current?.hasSelection() ? termRef.current.getSelection().trim() : ""
      if (!selection) return false
      const currentConnection = connectionRef.current
      const profile =
        currentConnection?.profileId && currentConnection.profileName
          ? { id: currentConnection.profileId, name: currentConnection.profileName }
          : undefined
      onSaveCommand?.(selection, profile)
    })
    return () => {
      unregisterFind()
      unregisterClear()
      unregisterReset()
      unregisterToggleSftp()
      unregisterZmodemSend()
      unregisterZmodemReceive()
      unregisterSaveSelection()
      unregisterFillSavedPassword()
      unregisterPreviousCommand()
      unregisterNextCommand()
      unregisterCopyCommandOutput()
    }
  }, [
    armZmodemManualTrigger,
    clearTerminalHistory,
    commandMarksRef,
    connectionRef,
    containerRef,
    copyLastCommandOutput,
    handleToggleSftpDrawer,
    isShortcutTargetRef,
    onSaveCommand,
    openSearch,
    registerHandler,
    resetTerminal,
    savedPasswordPromptActionsRef,
    termRef,
  ])
}
