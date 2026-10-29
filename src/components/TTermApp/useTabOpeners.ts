import { useCallback } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { AppConfig } from "@/lib/appConfig"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import { formatBytes, MAX_EDIT_FILE_BYTES } from "@/components/SftpDrawer/sftpDrawerUtils"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import { toast } from "@/hooks/use-toast"
import { defaultShellChoice, localShellConnection, type LocalShellChoice } from "@/lib/localShells"
import { nameLocalTab } from "@/lib/localTabTitle"
import { Tab } from "@/types/tab"

function formatRemoteFileConnectionLabel(connection?: Tab["connection"]): string | undefined {
  const profileName = connection?.profileName?.trim()
  if (profileName) {
    return profileName
  }

  const host = connection?.host?.trim()
  if (!host) {
    return undefined
  }

  const userHost = connection?.username ? `${connection.username}@${host}` : host
  return connection?.port && connection.port !== 22 ? `${userHost}:${connection.port}` : userHost
}

function getRemoteFileConnectionKey(connection?: Tab["connection"]): string {
  if (connection?.profileId) {
    return `profile:${connection.profileId}`
  }

  const host = connection?.host?.trim() ?? ""
  const username = connection?.username?.trim() ?? ""
  const port = connection?.port ?? 22
  return `target:${username}@${host}:${port}`
}

/** Opens tabs: a local terminal, a connection, or a remote file to edit. */
export function useTabOpeners(shell: AppShell, config: AppConfig) {
  const { t, tabs, addTab, setActiveTab } = shell

  const handleOpenLocalTerminal = useCallback(
    (choice?: LocalShellChoice, cwd?: string) => {
      void nameLocalTab({
        title: "",
        type: "terminal",
        isModified: false,
        connection: localShellConnection(choice ?? defaultShellChoice(config), cwd),
      }).then(addTab)
    },
    [addTab, config]
  )

  const handleConnect = useCallback(
    (connection: Omit<Tab, "id" | "isActive">) => {
      if (connection.type === "terminal") {
        void nameLocalTab(connection).then((tab) => addTab(buildTabFromConnection(tab)))
        return
      }
      addTab(buildTabFromConnection(connection))
    },
    [addTab]
  )

  const handleOpenRemoteFile = useCallback(
    (entry: SftpDirectoryEntry, sourceTabId: string, connection?: Tab["connection"]) => {
      const fileSize = entry.size ?? 0
      if (fileSize > MAX_EDIT_FILE_BYTES) {
        const message = `Remote file is too large to edit in tTerm (${fileSize} bytes, limit ${MAX_EDIT_FILE_BYTES} bytes)`
        toast({
          variant: "destructive",
          title: t("remoteFileEditor.openFailed", { defaultValue: "Failed to open remote file" }),
          description: `${message} (${formatBytes(fileSize)}, limit ${formatBytes(MAX_EDIT_FILE_BYTES)})`,
        })
        return
      }

      const host = connection?.host
      const connectionLabel = formatRemoteFileConnectionLabel(connection)
      const connectionKey = getRemoteFileConnectionKey(connection)
      const existingTab = tabs.find(
        (tab) =>
          tab.type === "remote-file-editor" &&
          tab.remoteFile?.path === entry.path &&
          (tab.remoteFile.connectionKey ?? getRemoteFileConnectionKey(tab.connection)) ===
            connectionKey
      )

      if (existingTab) {
        setActiveTab(existingTab.id)
        return
      }

      addTab({
        title: entry.name,
        type: "remote-file-editor",
        isModified: false,
        connection,
        remoteFile: {
          sourceTabId,
          profileId: connection?.profileId,
          profileName: connection?.profileName,
          connectionLabel,
          connectionKey,
          host,
          path: entry.path,
          fileName: entry.name,
          size: entry.size ?? 0,
          modifiedAt: entry.modifiedAt,
        },
      })
    },
    [addTab, setActiveTab, t, tabs]
  )

  return {
    handleOpenLocalTerminal,
    handleConnect,
    handleOpenRemoteFile,
  }
}
