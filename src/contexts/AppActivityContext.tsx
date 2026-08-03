import { createContext, useCallback, useContext, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { useConfirmDialog } from "@/components/ui/app-dialog"
import { relaunchApp } from "@/lib/updater"

interface AppActivityContextValue {
  activeTerminalSessionCount: number
  requestAppRestart: () => Promise<void>
  setActiveTerminalSessionCount: (count: number) => void
}

const AppActivityContext = createContext<AppActivityContextValue | null>(null)

export function AppActivityProvider({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [activeTerminalSessionCount, setActiveTerminalSessionCount] = useState(0)

  const requestAppRestart = useCallback(async () => {
    if (activeTerminalSessionCount > 0) {
      const confirmed = await confirm({
        title: t("updates.restartSessionsTitle", {
          defaultValue: "Restart and interrupt active sessions?",
        }),
        description: t("updates.restartSessionsDesc", {
          defaultValue:
            "Restarting tTerm will interrupt {{count}} active terminal session(s) and any running commands.",
          count: activeTerminalSessionCount,
        }),
        confirmText: t("updates.restartAnyway", { defaultValue: "Restart anyway" }),
        cancelText: t("common.cancel", { defaultValue: "Cancel" }),
        defaultAction: "cancel",
        variant: "destructive",
      })
      if (!confirmed) {
        return
      }
    }

    await relaunchApp()
  }, [activeTerminalSessionCount, confirm, t])

  const value = useMemo(
    () => ({ activeTerminalSessionCount, requestAppRestart, setActiveTerminalSessionCount }),
    [activeTerminalSessionCount, requestAppRestart]
  )

  return (
    <AppActivityContext.Provider value={value}>
      {children}
      <ConfirmDialog />
    </AppActivityContext.Provider>
  )
}

export function useAppActivity() {
  const context = useContext(AppActivityContext)
  if (!context) {
    throw new Error("useAppActivity must be used within AppActivityProvider")
  }
  return context
}
