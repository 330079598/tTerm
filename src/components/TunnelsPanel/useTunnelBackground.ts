import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useRunningTunnelCount } from "@/components/TunnelsPanel/useRunningTunnelCount"
import { useTunnelAutoStart } from "@/components/TunnelsPanel/useTunnelAutoStart"
import { useTunnelNotifications } from "@/components/TunnelsPanel/useTunnelNotifications"
import { useTunnelQuitGuard } from "@/components/TunnelsPanel/useTunnelQuitGuard"

/**
 * What tunnels need from the app outside their page: starting the automatic
 * ones once `ready`, asking before quitting with tunnels running, and their
 * notifications. Returns how many are running.
 */
export function useTunnelBackground(
  confirm: ReturnType<typeof useConfirmDialog>["confirm"],
  ready: boolean
) {
  useTunnelAutoStart(ready)
  useTunnelQuitGuard(confirm)
  useTunnelNotifications()
  return useRunningTunnelCount()
}
