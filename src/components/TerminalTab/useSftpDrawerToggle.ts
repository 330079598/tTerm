import React, { type Dispatch, type SetStateAction, useCallback, useEffect, useRef } from "react"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { TerminalTabProps } from "@/components/TerminalTab/types"

type UseSftpDrawerToggleOptions = Pick<TerminalTabProps, "isActive"> &
  Pick<TerminalTabRefs, "termRef"> & {
    showSftpDrawer: boolean
    setShowSftpDrawer: Dispatch<SetStateAction<boolean>>
  }

/** Opening and closing the SFTP drawer, and taking focus back when it closes. */
export function useSftpDrawerToggle({
  isActive,
  termRef,
  showSftpDrawer,
  setShowSftpDrawer,
}: UseSftpDrawerToggleOptions) {
  const handleConnectionHeaderMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (!showSftpDrawer) {
        return
      }

      const target = event.target as HTMLElement | null
      if (target?.closest("button")) {
        return
      }

      setShowSftpDrawer(false)
    },
    [setShowSftpDrawer, showSftpDrawer]
  )

  const handleToggleSftpDrawer = useCallback(() => {
    setShowSftpDrawer((current) => !current)
  }, [setShowSftpDrawer])

  // Closing the SFTP drawer makes it inert, which drops the focus held in it.
  const sftpDrawerWasShownRef = useRef(showSftpDrawer)
  useEffect(() => {
    const wasShown = sftpDrawerWasShownRef.current
    sftpDrawerWasShownRef.current = showSftpDrawer
    if (!wasShown || showSftpDrawer || !isActive) return
    const focused = document.activeElement
    if (!focused || focused === document.body || focused.closest(".sftp-drawer")) {
      termRef.current?.focus()
    }
  }, [isActive, showSftpDrawer, termRef])

  return {
    handleConnectionHeaderMouseDown,
    handleToggleSftpDrawer,
  }
}
