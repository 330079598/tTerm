import ReactDOM from "react-dom/client"

import App from "@/App"
import { DEFAULT_TERMINAL_FONT_FAMILY } from "@/contexts/ConfigContext"
import { initCanvasFontHost, updateCanvasFontHostFont } from "@/lib/canvasFontHost"
import { onAppReady } from "@/lib/startup"
import { preloadTheme } from "@/lib/themePreloader"

import "@/i18n/config"

// Ensure any canvas created for font rasterization is attached to the DOM (fixes WebKit font fallback)
initCanvasFontHost()
// Warm up WebKit's font resolution with the default font chain before React mounts;
// the configured font is re-applied per terminal before its renderer activates.
updateCanvasFontHostFont(DEFAULT_TERMINAL_FONT_FAMILY)

// Preload theme before React mounts to prevent flash
preloadTheme()

// Mount React application
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />)

let disposeReadyListener = () => {}
disposeReadyListener = onAppReady(() => {
  hideSplashScreen()
  disposeReadyListener()
})

function hideSplashScreen() {
  const splash = document.getElementById("splash-screen")
  if (splash) {
    splash.classList.add("hidden")
    // Remove element after transition
    setTimeout(() => {
      splash.remove()
    }, 300)
  }
}
