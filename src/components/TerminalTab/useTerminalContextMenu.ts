import type { TFunction } from "i18next"
import React, { useState, type Dispatch, type SetStateAction, useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { AppConfig } from "@/lib/appConfig"
import type { useTerminalClipboard } from "@/components/TerminalTab/useTerminalClipboard"
import { releaseClearedScrollback } from "@/components/TerminalTab/scrollbackMemory"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { toast } from "@/hooks/use-toast"
import { terminalBufferText, terminalTextFileName } from "@/lib/terminalText"
import { resetTerminalState } from "@/components/TerminalTab/terminalReset"
import { toErrorMessage } from "@/lib/utils"
import type { TabContextMenuAction } from "@/types/tab"

type UseTerminalContextMenuOptions = Pick<
  TerminalTabProps,
  "tabId" | "connection" | "onSaveCommand" | "onOpenCommandLibrary"
> &
  Pick<
    TerminalTabRefs,
    "termRef" | "commandMarksRef" | "configScrollbackLinesRef" | "connectionRef"
  > & { config: AppConfig; t: TFunction; sessionNonce: number } & {
    showSftpDrawer: boolean
    setShowSftpDrawer: Dispatch<SetStateAction<boolean>>
  } & Pick<ReturnType<typeof useTerminalClipboard>, "pasteFromClipboard">

/**
 * The terminal's context menu and the actions it offers: copying, pasting,
 * saving the contents, ZMODEM, resetting and clearing.
 */
export function useTerminalContextMenu({
  tabId,
  connection,
  onSaveCommand,
  onOpenCommandLibrary,
  termRef,
  commandMarksRef,
  configScrollbackLinesRef,
  connectionRef,
  config,
  t,
  sessionNonce,
  showSftpDrawer,
  setShowSftpDrawer,
  pasteFromClipboard,
}: UseTerminalContextMenuOptions) {
  const [terminalContextMenu, setTerminalContextMenu] = useState<{
    x: number
    y: number
    selection: string
    hasCommandOutput: boolean
  } | null>(null)
  const handleTerminalContextMenu = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      event.preventDefault()
      const term = termRef.current
      term?.focus()
      if (showSftpDrawer) setShowSftpDrawer(false)
      // Shift+right-click still opens the menu.
      if (config.right_click_paste && !event.shiftKey) {
        void pasteFromClipboard()
        return
      }
      setTerminalContextMenu({
        x: event.clientX,
        y: event.clientY,
        selection: term?.hasSelection() ? term.getSelection() : "",
        hasCommandOutput: commandMarksRef.current?.lastOutputRange() != null,
      })
    },
    [
      commandMarksRef,
      config.right_click_paste,
      pasteFromClipboard,
      setShowSftpDrawer,
      showSftpDrawer,
      termRef,
    ]
  )

  const clearTerminalHistory = useCallback(() => {
    const term = termRef.current
    if (!term) return

    term.clear()
    releaseClearedScrollback(term, configScrollbackLinesRef.current)
    term.scrollToBottom()
    term.clearSelection()
    term.focus()
  }, [configScrollbackLinesRef, termRef])

  const resetTerminal = useCallback(async () => {
    const term = termRef.current
    if (!term) return

    // Checked before anything is written: only a shell waiting at its prompt
    // takes Ctrl+L as "redraw" rather than as input.
    const atPrompt = commandMarksRef.current?.isAtPrompt() ?? false
    resetTerminalState(term)
    term.scrollToBottom()
    term.focus()
    try {
      // A local Windows shell's console translates output before xterm sees
      // it, so it needs a reset of its own (the backend skips everything
      // else). It must land before the redraw, or that would be garbled too.
      await invoke("reset_pty_console", { tabId, sessionNonce })
    } catch (error) {
      console.error("Failed to reset the shell's console:", error)
      toast({
        title: t("terminalContext.resetFailedTitle"),
        description: String(error),
        variant: "destructive",
      })
      return
    }
    // Characters already on screen were translated when printed, so a reset
    // cannot change them; the shell's redraw replaces them instead.
    if (atPrompt) {
      invoke("write_pty", { tabId, sessionNonce, data: "\x0c" }).catch(console.error)
    } else {
      // Without shell integration an SSH session can never tell it is at a
      // prompt; say where to turn it on.
      const currentConnection = connectionRef.current
      const withoutIntegration =
        currentConnection?.type === "ssh" && !currentConnection.shellIntegration
      toast({
        title: t("terminalContext.resetDoneTitle"),
        description: withoutIntegration
          ? `${t("terminalContext.resetDoneDescription")} ${t("terminalContext.resetShellIntegrationHint")}`
          : t("terminalContext.resetDoneDescription"),
      })
    }
  }, [commandMarksRef, connectionRef, sessionNonce, t, tabId, termRef])

  /** Selects and copies the output of the last command the shell marked. */
  const copyLastCommandOutput = useCallback(async () => {
    const marks = commandMarksRef.current
    const text = marks?.lastOutputText()
    if (!marks || text == null) return false
    if (!text) {
      toast({ title: t("terminalContext.commandOutputEmpty") })
      return true
    }
    marks.selectLastOutput()
    try {
      await invoke("plugin:clipboard-manager|write_text", { text })
    } catch (error) {
      console.error("Failed to copy command output:", error)
      toast({
        title: t("terminalContext.copyFailedTitle"),
        description: t("terminalContext.copyFailedDescription"),
        variant: "destructive",
      })
    }
    return true
  }, [commandMarksRef, t])

  /** Saves the terminal's text, scrollback included, to a file the user picks. */
  const saveTerminalContents = useCallback(async () => {
    const term = termRef.current
    if (!term) return
    const contents = terminalBufferText(term.buffer.normal)
    const name =
      connection?.profileName || connection?.host || t("terminalContext.contentsFileName")
    const path = await saveFileDialog({
      defaultPath: terminalTextFileName(name, new Date()),
      filters: [{ name: t("terminalContext.textFiles"), extensions: ["txt", "log"] }],
    }).catch(() => null)
    if (!path) return
    try {
      await invoke("save_terminal_contents", { path, contents })
      toast({ title: t("terminalContext.contentsSaved"), description: path })
    } catch (error) {
      toast({
        title: t("terminalContext.contentsSaveFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }, [connection?.host, connection?.profileName, t, termRef])

  const armZmodemManualTrigger = useCallback(
    async (direction: "send" | "receive") => {
      try {
        await invoke("zmodem_arm_manual_detect", { tabId, sessionNonce })
        toast({
          title: t(`zmodem.manual.${direction}ArmedTitle`),
          description: t(`zmodem.manual.${direction}ArmedDescription`),
        })
      } catch (error) {
        console.error("Failed to arm ZMODEM manual trigger:", error)
        toast({
          title: t("zmodem.manual.armFailedTitle"),
          description: String(error),
          variant: "destructive",
        })
      }
    },
    [sessionNonce, t, tabId]
  )

  const handleTerminalMenuAction = useCallback(
    async (action: string) => {
      if (action === "zmodem-send") {
        await armZmodemManualTrigger("send")
        return
      }
      if (action === "zmodem-receive") {
        await armZmodemManualTrigger("receive")
        return
      }
      const term = termRef.current
      const selection = terminalContextMenu?.selection.trim() ?? ""
      if (action === "clear-history") {
        clearTerminalHistory()
        return
      }
      if (action === "reset-terminal") {
        await resetTerminal()
        return
      }
      if (action === "copy-command-output") {
        await copyLastCommandOutput()
        return
      }
      if (action === "save-contents") {
        await saveTerminalContents()
        return
      }
      if (action === "copy" && selection) {
        try {
          await invoke("plugin:clipboard-manager|write_text", { text: selection })
          term?.clearSelection()
        } catch (error) {
          console.error("Failed to copy terminal selection:", error)
          toast({
            title: t("terminalContext.copyFailedTitle"),
            description: t("terminalContext.copyFailedDescription"),
            variant: "destructive",
          })
        }
        return
      }
      if (action === "save" && selection) {
        const profile =
          connection?.profileId && connection.profileName
            ? { id: connection.profileId, name: connection.profileName }
            : undefined
        onSaveCommand?.(selection, profile)
        return
      }
      if (action === "find") {
        onOpenCommandLibrary?.(selection || undefined)
        return
      }
      if (action === "paste") {
        await pasteFromClipboard()
      }
    },
    [
      armZmodemManualTrigger,
      clearTerminalHistory,
      connection,
      copyLastCommandOutput,
      onOpenCommandLibrary,
      onSaveCommand,
      pasteFromClipboard,
      resetTerminal,
      saveTerminalContents,
      t,
      terminalContextMenu,
      termRef,
    ]
  )

  const terminalMenuActions: TabContextMenuAction[] = [
    {
      action: "copy",
      label: t("terminalContext.copy"),
      icon: "copy",
      disabled: !terminalContextMenu?.selection,
    },
    {
      action: "copy-command-output",
      label: t("terminalContext.copyCommandOutput"),
      icon: "copy",
      shortcut: "terminal.copyLastCommandOutput",
      disabled: !terminalContextMenu?.hasCommandOutput,
    },
    {
      action: "save",
      label: t("terminalContext.saveCommand"),
      icon: "star",
      shortcut: "terminal.saveSelection",
      disabled: !terminalContextMenu?.selection,
    },
    { separator: true, action: "separator", label: "" },
    { action: "paste", label: t("terminalContext.paste"), icon: "paste" },
    {
      action: "find",
      label: t("terminalContext.findCommand"),
      icon: "search",
      shortcut: "commandLibrary.open",
    },
    { separator: true, action: "separator", label: "" },
    {
      action: "zmodem-send",
      label: t("terminalContext.zmodemSend"),
      icon: "upload",
      shortcut: "zmodem.sendFiles",
    },
    {
      action: "zmodem-receive",
      label: t("terminalContext.zmodemReceive"),
      icon: "download",
      shortcut: "zmodem.receiveFiles",
    },
    { separator: true, action: "separator", label: "" },
    { action: "save-contents", label: t("terminalContext.saveContents"), icon: "file-down" },
    {
      action: "reset-terminal",
      label: t("terminalContext.resetTerminal"),
      icon: "rotate-ccw",
      shortcut: "terminal.reset",
    },
    {
      action: "clear-history",
      label: t("terminalContext.clearHistory"),
      icon: "x",
      shortcut: "terminal.clear",
    },
  ]

  return {
    terminalContextMenu,
    setTerminalContextMenu,
    handleTerminalContextMenu,
    clearTerminalHistory,
    resetTerminal,
    copyLastCommandOutput,
    armZmodemManualTrigger,
    handleTerminalMenuAction,
    terminalMenuActions,
  }
}
