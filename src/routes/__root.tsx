import { createRootRoute } from "@tanstack/react-router"
import { lazy } from "react"

import { TTermApp } from "@/components/TTermApp"
import { AppUpdateManager } from "@/components/AppUpdateManager"
import { ConfigProvider } from "@/contexts/ConfigContext"
import { KeymapProvider } from "@/contexts/KeymapContext"
import { AppActivityProvider } from "@/contexts/AppActivityContext"
import { ThemeProvider } from "@/contexts/ThemeContext"
import { TransferProvider } from "@/contexts/TransferContext"
import { UserVerificationProvider } from "@/contexts/UserVerificationContext"

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
      <UserVerificationProvider>
        <KeymapProvider>
          <ThemeProvider>
            <TransferProvider>
              <AppActivityProvider>
                <TTermApp />
                <AppUpdateManager />
              </AppActivityProvider>
              <TanStackRouterDevtools />
            </TransferProvider>
          </ThemeProvider>
        </KeymapProvider>
      </UserVerificationProvider>
    </ConfigProvider>
  )
}

export const Route = createRootRoute({ component: RootLayout })
