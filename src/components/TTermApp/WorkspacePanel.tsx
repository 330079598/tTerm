import React from "react"

import { ErrorBoundary } from "@/components/ErrorBoundary"
import { TerminalTab } from "@/components/TerminalTab"
import { useWorkspace } from "@/components/TTermApp/workspaceContext"
import { isPageTab, type Tab } from "@/types/tab"
import { withLocalCwd } from "@/lib/localTabTitle"

const RemoteFileEditor = React.lazy(() =>
  import("@/components/RemoteFileEditor").then((module) => ({
    default: module.RemoteFileEditor,
  }))
)

const TunnelsPanel = React.lazy(() =>
  import("@/components/TunnelsPanel").then((module) => ({
    default: module.TunnelsPanel,
  }))
)

const LogsPanel = React.lazy(() =>
  import("@/components/LogsPanel").then((module) => ({
    default: module.LogsPanel,
  }))
)

const SettingsPanel = React.lazy(() =>
  import("@/components/SettingsDialog").then((module) => ({
    default: module.SettingsPanel,
  }))
)

interface WorkspacePanelProps {
  tab: Tab
  /** The pane is in sight: its tab in the bar is the active one, and no zoom hides it. */
  isVisible: boolean
  /** The pane has been in sight since it opened. */
  hasBeenVisible: boolean
  /** The pane has the keyboard: the app's active tab. */
  isActive: boolean
}

/** What a pane shows: a terminal, a page or a remote file. */
export const WorkspacePanel = React.memo(function WorkspacePanel({
  tab,
  isVisible,
  hasBeenVisible,
  isActive,
}: WorkspacePanelProps) {
  const workspace = useWorkspace()

  const isSshConnection = tab.connection?.type === "ssh" || tab.type === "ssh"
  const shouldConnect =
    !isPageTab(tab) &&
    tab.type !== "remote-file-editor" &&
    (workspace.startupConnectionsReady || !isSshConnection) &&
    (workspace.startupSessionRestoreMode === "all" ||
      isVisible ||
      hasBeenVisible ||
      tab.hasConnected === true)

  // Pages render only while in sight, as they hold no session to keep.
  const showsPage = isVisible || !isPageTab(tab)

  return (
    <div className="workspace-panel" data-workspace-panel-id={tab.id}>
      {!showsPage ? null : tab.type === "settings" ? (
        <ErrorBoundary resetKey={tab.id} scope="settings">
          <React.Suspense fallback={null}>
            <SettingsPanel
              onConnectProfile={workspace.onConnectProfile}
              onEditProfile={workspace.onEditProfile}
              profilesRefreshKey={workspace.profilesRefreshKey}
            />
          </React.Suspense>
        </ErrorBoundary>
      ) : tab.type === "tunnels" ? (
        <ErrorBoundary resetKey={tab.id} scope="tunnels">
          <React.Suspense fallback={null}>
            <TunnelsPanel profilesRefreshKey={workspace.profilesRefreshKey} />
          </React.Suspense>
        </ErrorBoundary>
      ) : tab.type === "logs" ? (
        <ErrorBoundary resetKey={tab.id} scope="logs">
          <React.Suspense fallback={null}>
            <LogsPanel />
          </React.Suspense>
        </ErrorBoundary>
      ) : tab.type === "remote-file-editor" ? (
        <ErrorBoundary resetKey={tab.id} scope="remote-file-editor">
          <React.Suspense fallback={null}>
            <RemoteFileEditor
              isVisible={isVisible}
              isGlobalShortcutTarget={isActive}
              tab={tab}
              onTabUpdate={(updater) => workspace.updateTab(tab.id, updater)}
            />
          </React.Suspense>
        </ErrorBoundary>
      ) : (
        shouldConnect && (
          <ErrorBoundary resetKey={`${tab.id}:${tab.sessionNonce ?? 0}`} scope="terminal-tab">
            <TerminalTab
              tabId={tab.id}
              sessionNonce={tab.sessionNonce}
              isActive={isVisible}
              isGlobalShortcutTarget={isActive}
              connectionHeaderPinned={tab.connectionHeaderPinned}
              connection={tab.connection ?? { type: tab.type === "terminal" ? "terminal" : "ssh" }}
              shellName={tab.shellName}
              isBroadcastSource={workspace.broadcastSourceTabId === tab.id}
              liveBroadcastState={workspace.liveBroadcastState}
              onPauseBroadcast={workspace.onPauseBroadcast}
              onResumeBroadcast={workspace.onResumeBroadcast}
              onStopBroadcast={workspace.onStopBroadcast}
              onCwdChange={(cwd) =>
                workspace.updateTab(tab.id, (current) => withLocalCwd(current, cwd))
              }
              onConnectionStateChange={workspace.onTerminalConnectionStateChange}
              onInput={workspace.onTerminalInput}
              onCommandExecuted={workspace.onTerminalCommandExecuted}
              onAttention={workspace.onTerminalAttention}
              onOpenCommandLibrary={workspace.onOpenCommandLibrary}
              onSaveCommand={workspace.onSaveCommand}
              onSavedPasswordPromptChange={workspace.onTerminalSavedPasswordPromptChange}
              onSessionUnavailable={workspace.onTerminalSessionUnavailable}
              onReconnectRequest={() => workspace.handleReconnectTab(tab.id)}
              onOpenRemoteFile={workspace.onOpenRemoteFile}
              onPinConnectionHeader={() => workspace.handlePinConnectionHeader(tab.id)}
              onServerMonitorVisibilityChange={(visible) =>
                workspace.handleServerMonitorVisibilityChange(tab.id, visible)
              }
              onSensitivePrompt={workspace.onTerminalSensitivePrompt}
              onConnectionProgress={(progress) =>
                workspace.onTerminalConnectionProgress?.(tab.id, progress)
              }
              onUnpinConnectionHeader={() => workspace.handleUnpinConnectionHeader(tab.id)}
            />
          </ErrorBoundary>
        )
      )}
    </div>
  )
})
