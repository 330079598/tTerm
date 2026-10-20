import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import { terminalCdCommand } from "@/lib/terminalCd"

type UseCdInTerminalOptions = Pick<TerminalTabRefs, "termRef" | "commandMarksRef"> & {
  tabId: string
  sessionNonce: number
  setShowSftpDrawer: (show: boolean) => void
}

/**
 * Takes the tab's shell to a directory for the SFTP drawer's "Open in
 * terminal", then shows the terminal. The line goes to this session only, not
 * to the tabs it broadcasts to. Returns false while a program has the
 * terminal (an editor, a pager, a command shell integration saw start),
 * which would read the line instead.
 */
export function useCdInTerminal({
  tabId,
  sessionNonce,
  termRef,
  commandMarksRef,
  setShowSftpDrawer,
}: UseCdInTerminalOptions) {
  return useCallback(
    (path: string) => {
      const term = termRef.current
      const commandMarks = commandMarksRef.current
      if (!term || !commandMarks || commandMarks.isRunningProgram()) return false
      void invoke("write_pty", { tabId, sessionNonce, data: terminalCdCommand(path) }).catch(
        console.error
      )
      setShowSftpDrawer(false)
      term.focus()
      return true
    },
    [commandMarksRef, sessionNonce, setShowSftpDrawer, tabId, termRef]
  )
}
