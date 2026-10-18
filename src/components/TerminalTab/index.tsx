import "@/components/TerminalTab.css"
import React, { useCallback, useState } from "react"
import { useTranslation } from "react-i18next"

import { useTerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import { useSessionScopedState } from "@/components/TerminalTab/useSessionScopedState"
import { useTerminalTheme } from "@/components/TerminalTab/useTerminalTheme"
import { useTerminalFit } from "@/components/TerminalTab/useTerminalFit"
import { useTerminalConfigSync } from "@/components/TerminalTab/useTerminalConfigSync"
import { useTerminalActivation } from "@/components/TerminalTab/useTerminalActivation"
import { useConnectionNotices } from "@/components/TerminalTab/useConnectionNotices"
import { useSftpDrawerToggle } from "@/components/TerminalTab/useSftpDrawerToggle"
import { useTerminalClipboard } from "@/components/TerminalTab/useTerminalClipboard"
import { useTerminalFileDrop } from "@/components/TerminalTab/useTerminalFileDrop"
import { useTerminalContextMenu } from "@/components/TerminalTab/useTerminalContextMenu"
import { useTerminalShortcuts } from "@/components/TerminalTab/useTerminalShortcuts"
import { BroadcastControls } from "@/components/TerminalTab/BroadcastControls"
import { ContextMenu } from "@/components/ContextMenu"
import { SftpDrawer } from "@/components/SftpDrawer"
import { ConnectionHeader } from "@/components/TerminalTab/ConnectionHeader"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { SshAuthPromptDialog } from "@/components/TerminalTab/SshAuthPromptDialog"
import { JumpHostInfoDialog } from "@/components/TerminalTab/JumpHostInfoDialog"
import { SavedPasswordPromptBar } from "@/components/TerminalTab/SavedPasswordPromptBar"
import { ServerMonitorBar } from "@/components/TerminalTab/ServerMonitorBar"
import { TerminalSearchBar } from "@/components/TerminalTab/TerminalSearchBar"
import { useTerminalSearch } from "@/components/TerminalTab/useTerminalSearch"
import { useTerminalLifecycle } from "@/components/TerminalTab/useTerminalLifecycle"
import { useZmodemTransfers } from "@/components/TerminalTab/useZmodemTransfers"
import {
  useConnectionStateReport,
  useServerMonitorToggle,
} from "@/components/TerminalTab/useTerminalTabStatus"
import type { SavedPasswordPromptState, TerminalTabProps } from "@/components/TerminalTab/types"
import { useConfig } from "@/contexts/ConfigContext"
import { useKeymap } from "@/contexts/KeymapContext"

export const TerminalTab: React.FC<TerminalTabProps> = ({
  tabId,
  sessionNonce = 0,
  isActive,
  isGlobalShortcutTarget,
  workspacePanelApi,
  connectionHeaderPinned = true,
  connection,
  shellName,
  isBroadcastSource = false,
  liveBroadcastState = "idle",
  onConnectionStateChange,
  onInput,
  onCommandExecuted,
  onOpenCommandLibrary,
  onSaveCommand,
  onPidChange,
  onCwdChange,
  onReconnectRequest,
  onSavedPasswordPromptChange,
  onSessionUnavailable,
  onSensitivePrompt,
  onConnectionProgress,
  onAttention,
  onOpenRemoteFile,
  onPinConnectionHeader,
  onPauseBroadcast,
  onResumeBroadcast,
  onStopBroadcast,
  onServerMonitorVisibilityChange,
  onUnpinConnectionHeader,
}) => {
  const { config, saveConfig } = useConfig()
  const refs = useTerminalTabRefs({
    tabId,
    isActive,
    isGlobalShortcutTarget,
    connection,
    onPidChange,
    onCwdChange,
    onInput,
    onCommandExecuted,
    onReconnectRequest,
    onSavedPasswordPromptChange,
    onSessionUnavailable,
    onSensitivePrompt,
    onConnectionProgress,
    onAttention,
    config,
  })
  const {
    containerRef,
    surfaceRef,
    shellRef,
    termRef,
    searchAddonRef,
    isActiveRef,
    waitingForReconnectRef,
    savedPasswordPromptActionsRef,
  } = refs
  const { t } = useTranslation()
  const sessionResetKey = `${tabId}:${sessionNonce}:${connection?.type ?? "terminal"}`
  const [savedPasswordPrompt, setSavedPasswordPrompt] = useState<SavedPasswordPromptState | null>(
    null
  )

  const {
    hostKeyPrompt,
    connectionState,
    connectionProgress,
    setHostKeyPrompt,
    setConnectionState,
    setConnectionProgress,
  } = useSessionScopedState({ sessionResetKey })
  const [showSftpDrawer, setShowSftpDrawer] = useState(false)
  const { showServerMonitor, handleToggleServerMonitor } = useServerMonitorToggle(
    connection,
    onServerMonitorVisibilityChange
  )

  const {
    closeSearch,
    handleSearchDragStart,
    isSearchDragging,
    isSearchOpen,
    openSearch,
    runSearch,
    searchInputRef,
    searchOptions,
    searchPosition,
    searchQuery,
    searchResults,
    setSearchQuery,
    setSearchResults,
    toggleSearchOption,
  } = useTerminalSearch({
    isActiveRef,
    searchAddonRef,
    setShowSftpDrawer,
    surfaceRef,
    termRef,
  })

  const { bindings, registerHandler } = useKeymap()

  const { windowBlur, connectionPalette, resolveTerminalTheme, terminalThemeRef } =
    useTerminalTheme({
      connection,
      config,
    })

  const { fitTerminalOnly, fitAndSyncPty, scheduleFitDuringResize } = useTerminalFit({
    ...refs,
    tabId,
    sessionNonce,
  })

  useTerminalLifecycle({
    ...refs,
    fitTerminalOnly,
    terminalThemeRef,
    hiddenRendererReleaseSecs: config.hidden_renderer_release_secs,
    isActive,
    setSavedPasswordPrompt,
    scheduleFitDuringResize,
    setConnectionState,
    setHostKeyPrompt,
    setConnectionProgress,
    setSearchResults,
    sessionNonce,
    tabId,
    terminalRenderer: config.terminal_renderer,
  })

  useZmodemTransfers({ tabId, sessionNonce })

  useTerminalConfigSync({
    ...refs,
    config,
    sessionNonce,
    scheduleFitDuringResize,
    fitAndSyncPty,
    resolveTerminalTheme,
  })

  useConnectionStateReport({ tabId, sessionNonce, connectionState, onConnectionStateChange })

  useTerminalActivation({
    ...refs,
    workspacePanelApi,
    isActive,
    tabId,
    isGlobalShortcutTarget,
    scheduleFitDuringResize,
  })

  const {
    jumpHostInfoOpen,
    dontShowJumpHostInfoAgain,
    setDontShowJumpHostInfoAgain,
    handleJumpHostInfoOpenChange,
  } = useConnectionNotices({
    ...refs,
    connection,
    config,
    saveConfig,
    sessionResetKey,
    t,
    connectionProgress,
  })

  const handleReconnect = useCallback(() => {
    onSessionUnavailable?.(tabId, sessionNonce, false)
    waitingForReconnectRef.current = false
    setConnectionState("connecting")
    onReconnectRequest?.()
  }, [
    onReconnectRequest,
    onSessionUnavailable,
    sessionNonce,
    setConnectionState,
    tabId,
    waitingForReconnectRef,
  ])

  const { handleConnectionHeaderMouseDown, handleToggleSftpDrawer } = useSftpDrawerToggle({
    ...refs,
    isActive,
    showSftpDrawer,
    setShowSftpDrawer,
  })

  const { PasteConfirmDialog, pasteFromClipboard } = useTerminalClipboard({
    ...refs,
    isBroadcastSource,
    liveBroadcastState,
    config,
    t,
  })

  useTerminalFileDrop({ ...refs, connection, isActive, shellName })

  const {
    terminalContextMenu,
    setTerminalContextMenu,
    handleTerminalContextMenu,
    clearTerminalHistory,
    resetTerminal,
    copyLastCommandOutput,
    armZmodemManualTrigger,
    handleTerminalMenuAction,
    terminalMenuActions,
  } = useTerminalContextMenu({
    ...refs,
    tabId,
    connection,
    onSaveCommand,
    onOpenCommandLibrary,
    config,
    t,
    sessionNonce,
    showSftpDrawer,
    setShowSftpDrawer,
    pasteFromClipboard,
  })

  useTerminalShortcuts({
    ...refs,
    onSaveCommand,
    registerHandler,
    openSearch,
    handleToggleSftpDrawer,
    clearTerminalHistory,
    resetTerminal,
    armZmodemManualTrigger,
    copyLastCommandOutput,
  })

  const terminalPaddingStyle = {
    "--terminal-padding-left": `${config.terminal_padding_left_px}px`,
    "--terminal-padding-right": `${config.terminal_padding_right_px}px`,
    "--terminal-padding-bottom": `${config.terminal_padding_bottom_px}px`,
  } as React.CSSProperties

  return (
    <div
      ref={shellRef}
      className={`terminal-tab-shell ${isActive ? "is-active" : ""} ${isBroadcastSource ? "is-broadcast-source" : ""}`}
      aria-label={isBroadcastSource ? t("broadcast.sourceTerminal") : undefined}
    >
      <ConnectionHeader
        tabId={tabId}
        sessionNonce={sessionNonce}
        connection={connection}
        connectionHeaderPinned={connectionHeaderPinned}
        connectionState={connectionState}
        connectionProgress={connectionProgress}
        onBackgroundMouseDown={handleConnectionHeaderMouseDown}
        onPinConnectionHeader={onPinConnectionHeader}
        onReconnect={handleReconnect}
        onToggleServerMonitor={handleToggleServerMonitor}
        onToggleSftpDrawer={handleToggleSftpDrawer}
        onUnpinConnectionHeader={onUnpinConnectionHeader}
        serverMonitorVisible={showServerMonitor}
      />

      {isBroadcastSource && liveBroadcastState !== "idle" && (
        <BroadcastControls
          liveBroadcastState={liveBroadcastState}
          fontFamily={config.font_family}
          onPauseBroadcast={onPauseBroadcast}
          onResumeBroadcast={onResumeBroadcast}
          onStopBroadcast={onStopBroadcast}
        />
      )}

      <div
        ref={surfaceRef}
        className="terminal-surface"
        style={{
          ...terminalPaddingStyle,
          // The padding around the grid takes the connection's background too.
          ...(connectionPalette && !windowBlur ? { background: connectionPalette.background } : {}),
        }}
      >
        <SftpDrawer
          tabId={tabId}
          visible={showSftpDrawer}
          isGlobalShortcutTarget={isGlobalShortcutTarget}
          connection={connection}
          onClose={() => setShowSftpDrawer(false)}
          onOpenRemoteFile={onOpenRemoteFile}
        />
        {isSearchOpen && (
          <TerminalSearchBar
            isSearchDragging={isSearchDragging}
            onClose={closeSearch}
            onDragStart={handleSearchDragStart}
            onRunSearch={(direction) => runSearch(direction)}
            onToggleOption={toggleSearchOption}
            searchInputRef={searchInputRef}
            searchOptions={searchOptions}
            searchPosition={searchPosition}
            searchQuery={searchQuery}
            searchResults={searchResults}
            setSearchQuery={setSearchQuery}
            t={t}
          />
        )}

        <div
          ref={containerRef}
          className="terminal-host"
          data-allow-context-menu
          onMouseDown={() => {
            termRef.current?.focus()
            if (showSftpDrawer) {
              setShowSftpDrawer(false)
            }
          }}
          onContextMenu={handleTerminalContextMenu}
          style={{
            width: "100%",
            height: "100%",
            overflow: "hidden",
            backgroundColor: windowBlur
              ? "transparent"
              : (connectionPalette?.background ?? "hsl(var(--background))"),
          }}
        />

        {terminalContextMenu && (
          <ContextMenu
            x={terminalContextMenu.x}
            y={terminalContextMenu.y}
            actions={terminalMenuActions}
            onAction={(action) => void handleTerminalMenuAction(action)}
            onClose={() => setTerminalContextMenu(null)}
          />
        )}

        {savedPasswordPrompt && (
          <SavedPasswordPromptBar
            connection={connection}
            fillShortcut={bindings["terminal.fillSavedPassword"]?.[0]}
            onDismiss={() => {
              savedPasswordPromptActionsRef.current?.dismiss()
              termRef.current?.focus()
            }}
            onFill={() => {
              savedPasswordPromptActionsRef.current?.fill()
              termRef.current?.focus()
            }}
            state={savedPasswordPrompt}
          />
        )}

        <HostKeyPromptDialog hostKeyPrompt={hostKeyPrompt} setHostKeyPrompt={setHostKeyPrompt} />
        <SshAuthPromptDialog
          tabId={tabId}
          visible={isActive}
          remembersPasswords={connection?.authMethod === "auto"}
        />
        <PasteConfirmDialog />
        <JumpHostInfoDialog
          connection={connection}
          dontShowAgain={dontShowJumpHostInfoAgain}
          onDontShowAgainChange={setDontShowJumpHostInfoAgain}
          onOpenChange={handleJumpHostInfoOpenChange}
          open={jumpHostInfoOpen}
        />
      </div>
      <ServerMonitorBar
        active={isActive}
        connection={connection}
        connectionState={connectionState}
        sessionNonce={sessionNonce}
        tabId={tabId}
        t={t}
        visible={showServerMonitor}
      />
    </div>
  )
}
