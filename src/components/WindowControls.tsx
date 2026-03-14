import { getCurrentWindow } from "@tauri-apps/api/window"
import { platform } from "@tauri-apps/plugin-os"
import { Minus, Square, X } from "lucide-react"
import { useEffect, useState } from "react"

const appWindow = getCurrentWindow()

export function WindowControls() {
  const [os] = useState<string>(() => platform())
  const [isMaximized, setIsMaximized] = useState(false)

  useEffect(() => {
    // listen window resized
    const unlisten = appWindow.onResized(async () => {
      setIsMaximized(await appWindow.isMaximized())
    })

    // init maximized state
    appWindow.isMaximized().then(setIsMaximized)

    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  const handleMinimize = async () => await appWindow.minimize()
  const handleMaximize = async () => await appWindow.toggleMaximize()
  const handleClose = async () => await appWindow.close()

  // macOS style - hide controls as they're handled by the system
  if (os === "macos") {
    return null
  }

  // Windows/Linux style
  return (
    <div className="window-controls">
      <button onClick={handleMinimize} className="window-control" title="Minimize">
        <Minus size={16} />
      </button>
      <button
        onClick={handleMaximize}
        className="window-control"
        title={isMaximized ? "Restore" : "Maximize"}
      >
        <Square size={14} />
      </button>
      <button onClick={handleClose} className="window-control close" title="Close">
        <X size={16} />
      </button>
    </div>
  )
}
