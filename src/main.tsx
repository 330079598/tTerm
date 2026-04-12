import ReactDOM from "react-dom/client"

import App from "@/App"
import { preloadTheme } from "@/lib/themePreloader"

import "@/i18n/config"

// Preload theme before React mounts to prevent flash
preloadTheme()

// Mount React application
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />)

// Hide splash screen after React renders
// Use requestIdleCallback for better performance
if ("requestIdleCallback" in window) {
  requestIdleCallback(() => {
    hideSplashScreen()
  })
} else {
  // Fallback for browsers without requestIdleCallback
  setTimeout(hideSplashScreen, 100)
}

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
