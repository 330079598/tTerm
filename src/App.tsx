import "@/App.css"
import { RouterProvider, createRouter } from "@tanstack/react-router"
import { invoke } from "@tauri-apps/api/core"
import { useEffect } from "react"
import { ErrorBoundary } from "@/components/ErrorBoundary"
import { Toaster } from "@/components/ui/toaster"
import { readBackupFrontendState } from "@/lib/backupFrontendState"

// Import the generated route tree
import { routeTree } from "@/routeTree.gen"

const WEBDAV_BACKUP_CHECK_INTERVAL_MS = 30 * 60 * 1_000

// Create a new router instance
const router = createRouter({ routeTree })

// Register the router instance for type safety
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}

function App() {
  useEffect(() => {
    const runWebDavBackup = () =>
      invoke("run_webdav_backup", {
        frontendState: readBackupFrontendState(),
        force: false,
      }).catch((error) => console.error("WebDAV backup failed:", error))
    const timer = window.setTimeout(() => {
      invoke("run_due_automatic_backup", {
        frontendState: readBackupFrontendState(),
        force: false,
      }).catch((error) => console.error("Automatic backup failed:", error))
      void runWebDavBackup()
    }, 2_000)
    // Checked again while the app stays open, which also catches a run that
    // was skipped because saved passwords were still locked.
    const interval = window.setInterval(runWebDavBackup, WEBDAV_BACKUP_CHECK_INTERVAL_MS)
    return () => {
      window.clearTimeout(timer)
      window.clearInterval(interval)
    }
  }, [])

  return (
    <>
      <ErrorBoundary scope="app">
        <RouterProvider router={router} />
      </ErrorBoundary>
      <Toaster />
    </>
  )
}

export default App
