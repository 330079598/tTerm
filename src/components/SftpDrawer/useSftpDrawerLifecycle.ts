import { invoke } from "@tauri-apps/api/core"
import { useEffect, type RefObject } from "react"

import { preloadMaterialIcons } from "@/components/SftpDrawer/SftpEntryIcon"

/**
 * Releases the tab's SFTP connections when the drawer goes, and keeps the
 * hidden drawer out of focus and the accessibility tree.
 */
export function useSftpDrawerLifecycle(
  tabId: string,
  visible: boolean,
  drawerRef: RefObject<HTMLDivElement | null>
) {
  // The drawer lives as long as its tab, so unmounting means the tab closed:
  // let the backend close the tab's pooled SFTP connections.
  useEffect(
    () => () => {
      void invoke("sftp_release_tab_connections", { tabId }).catch(console.error)
    },
    [tabId]
  )

  useEffect(() => {
    const drawer = drawerRef.current
    if (!drawer) {
      return
    }

    if (visible) {
      drawer.removeAttribute("inert")
      // Usually done before the first directory listing arrives.
      preloadMaterialIcons().catch(console.error)
    } else {
      drawer.setAttribute("inert", "")
    }
  }, [drawerRef, visible])
}
