import ReactDOM from "react-dom/client"

import App from "@/App"
import { initCanvasFontHost } from "@/lib/canvasFontHost"
import { onAppReady } from "@/lib/startup"
import { preloadTheme } from "@/lib/themePreloader"

import "@/i18n/config"

// Ensure any canvas created for font rasterization is attached to the DOM (fixes WebKit font fallback)
initCanvasFontHost()

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
