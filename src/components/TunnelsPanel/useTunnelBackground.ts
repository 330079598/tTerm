import { useRunningTunnelCount } from "@/components/TunnelsPanel/useRunningTunnelCount"
import { useTunnelAutoStart } from "@/components/TunnelsPanel/useTunnelAutoStart"
import { useTunnelNotifications } from "@/components/TunnelsPanel/useTunnelNotifications"

/**
 * What tunnels need from the app outside their page: starting the automatic
 * ones once `ready`, and their notifications. Returns how many are running.
 */
export function useTunnelBackground(ready: boolean) {
  useTunnelAutoStart(ready)
  useTunnelNotifications()
  return useRunningTunnelCount()
}
