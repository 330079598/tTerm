import { useCallback, useEffect, useRef, useState } from "react"

import {
  dropSftpHistoryEntry,
  EMPTY_SFTP_HISTORY,
  landSftpHistory,
  recordSftpVisit,
  sftpHistoryTarget,
  type SftpHistory,
  type SftpHistoryStep,
} from "@/components/SftpDrawer/sftpHistory"
import type { LoadSftpDirectory } from "@/components/SftpDrawer/types"
import { hasOpenShortcutBlockingLayer, useKeymap } from "@/contexts/KeymapContext"

/**
 * History state the drawer's `loadDirectory` reports into. Kept apart from
 * the navigation hook because navigation needs `loadDirectory`, which in
 * turn needs these recorders.
 */
export function useSftpHistory() {
  const [history, setHistory] = useState<SftpHistory>(EMPTY_SFTP_HISTORY)

  /** A listing loaded; `historyIndex` is set when back/forward asked for it. */
  const recordLoaded = useCallback((path: string, historyIndex?: number) => {
    setHistory((current) =>
      historyIndex === undefined
        ? recordSftpVisit(current, path)
        : landSftpHistory(current, historyIndex, path)
    )
  }, [])

  /** A listing failed; a back/forward target that fails is dropped. */
  const recordFailed = useCallback((historyIndex?: number) => {
    if (historyIndex !== undefined) {
      setHistory((current) => dropSftpHistoryEntry(current, historyIndex))
    }
  }, [])

  return { history, recordFailed, recordLoaded }
}

interface UseSftpHistoryNavigationParams {
  contextMenuOpen: boolean
  dialogOpen: boolean
  drawerRef: React.RefObject<HTMLElement | null>
  history: SftpHistory
  isGlobalShortcutTarget: boolean
  isLoading: boolean
  loadDirectory: LoadSftpDirectory
  parentPath: string | null
  visible: boolean
}

export function useSftpHistoryNavigation({
  contextMenuOpen,
  dialogOpen,
  drawerRef,
  history,
  isGlobalShortcutTarget,
  isLoading,
  loadDirectory,
  parentPath,
  visible,
}: UseSftpHistoryNavigationParams) {
  const { registerHandler } = useKeymap()
  const sideButtonPressRef = useRef<{ button: number; blocked: boolean } | null>(null)

  const navigateHistory = useCallback(
    (step: SftpHistoryStep) => {
      const target = sftpHistoryTarget(history, step)
      if (target === null || isLoading) {
        return false
      }

      void loadDirectory(history.paths[target], { historyIndex: target })
      return true
    },
    [history, isLoading, loadDirectory]
  )

  const goBack = useCallback(() => navigateHistory(-1), [navigateHistory])
  const goForward = useCallback(() => navigateHistory(1), [navigateHistory])

  const goUp = useCallback(() => {
    if (!parentPath || isLoading) {
      return false
    }

    void loadDirectory(parentPath)
    return true
  }, [isLoading, loadDirectory, parentPath])

  useEffect(() => {
    const canNavigate = () => visible && isGlobalShortcutTarget && !dialogOpen
    const unregisterHandlers = [
      registerHandler("sftp.back", () => canNavigate() && goBack()),
      registerHandler("sftp.forward", () => canNavigate() && goForward()),
      registerHandler("sftp.up", () => canNavigate() && goUp()),
    ]
    return () => unregisterHandlers.forEach((unregister) => unregister())
  }, [dialogOpen, goBack, goForward, goUp, isGlobalShortcutTarget, registerHandler, visible])

  // Mouse side buttons: 3 is back, 4 is forward. Both the press and the
  // release are cancelled so WebView2 never runs its own page navigation.
  const handleSideButton = useCallback(
    (event: React.MouseEvent<HTMLElement>) => {
      if (event.button !== 3 && event.button !== 4) {
        return
      }

      event.preventDefault()
      // React bubbles events out of portaled dialogs too; only the drawer's
      // own surface navigates.
      const insideDrawer = drawerRef.current?.contains(event.target as Node) ?? false
      if (event.type === "mousedown") {
        // Decided on the press: an open menu closes itself on mousedown, so
        // by the release it is gone. As with the keyboard shortcuts, a click
        // that dismisses a menu or popover does not also navigate.
        sideButtonPressRef.current = insideDrawer
          ? {
              button: event.button,
              blocked: dialogOpen || contextMenuOpen || hasOpenShortcutBlockingLayer(document),
            }
          : null
        return
      }

      const press = sideButtonPressRef.current
      sideButtonPressRef.current = null
      if (insideDrawer && press?.button === event.button && !press.blocked) {
        navigateHistory(event.button === 3 ? -1 : 1)
      }
    },
    [contextMenuOpen, dialogOpen, drawerRef, navigateHistory]
  )

  return {
    canGoBack: sftpHistoryTarget(history, -1) !== null,
    canGoForward: sftpHistoryTarget(history, 1) !== null,
    canGoUp: parentPath !== null,
    goBack,
    goForward,
    goUp,
    handleSideButton,
  }
}
