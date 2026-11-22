import { useEffect } from "react"

import { TunnelsPanel } from "@/components/TunnelsPanel"
import { ConfigProvider } from "@/contexts/ConfigContext"
import { UserVerificationProvider } from "@/contexts/UserVerificationContext"
import "@/i18n/config"
import type { ParityScreenProps } from "@/parity/types"

/** The capture also waits for the fixture's empty state or last rule to appear. */
export function TunnelPanelReference({ onReady }: ParityScreenProps) {
  useEffect(onReady, [onReady])
  return (
    <ConfigProvider>
      <UserVerificationProvider>
        <TunnelsPanel />
      </UserVerificationProvider>
    </ConfigProvider>
  )
}
