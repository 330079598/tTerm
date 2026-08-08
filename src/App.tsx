import "@/App.css"
import { RouterProvider, createRouter } from "@tanstack/react-router"
import { invoke } from "@tauri-apps/api/core"
import { useEffect } from "react"
import { ErrorBoundary } from "@/components/ErrorBoundary"
import { Toaster } from "@/components/ui/toaster"
import { RECENT_COMMANDS_STORAGE_KEY } from "@/lib/recentCommands"

// Import the generated route tree
import { routeTree } from "@/routeTree.gen"

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
    const timer = window.setTimeout(() => {
      let customThemes: unknown[] = []
      let recentCommands: unknown[] = []
      let sftpColumnWidths: unknown = null
      try {
        const stored = JSON.parse(localStorage.getItem("custom-themes") ?? "[]") as unknown
        customThemes = Array.isArray(stored) ? stored : []
        const recent = JSON.parse(
          localStorage.getItem(RECENT_COMMANDS_STORAGE_KEY) ?? "[]"
        ) as unknown
        recentCommands = Array.isArray(recent) ? recent : []
        sftpColumnWidths = JSON.parse(
          localStorage.getItem("tterm.sftp.columnWidths") ?? "null"
        ) as unknown
      } catch {
        customThemes = []
        recentCommands = []
        sftpColumnWidths = null
      }
      invoke("run_due_automatic_backup", {
        frontendState: { customThemes, recentCommands, sftpColumnWidths },
        force: false,
      }).catch((error) => console.error("Automatic backup failed:", error))
    }, 2_000)
    return () => window.clearTimeout(timer)
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
