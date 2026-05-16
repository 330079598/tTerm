import { createRootRoute } from "@tanstack/react-router"
import { lazy } from "react"

import { TTermApp } from "@/components/TTermApp"
import { AppUpdateManager } from "@/components/AppUpdateManager"
import { ConfigProvider } from "@/contexts/ConfigContext"
import { ThemeProvider } from "@/contexts/ThemeContext"
import { TransferProvider } from "@/contexts/TransferContext"

const TanStackRouterDevtools = import.meta.env.DEV
  ? lazy(() =>
      import("@tanstack/react-router-devtools").then((m) => ({
        default: m.TanStackRouterDevtools,
      }))
    )
  : () => null

const RootLayout = () => {
  return (
    <ConfigProvider>
      <ThemeProvider>
        <TransferProvider>
          <TTermApp />
          <AppUpdateManager />
          <TanStackRouterDevtools />
        </TransferProvider>
      </ThemeProvider>
    </ConfigProvider>
  )
}

export const Route = createRootRoute({ component: RootLayout })
