// @vitest-environment jsdom

import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { renderHook, waitFor } from "@testing-library/react"
import { expect, it, vi } from "vitest"

import { useTunnelList } from "@/components/TunnelsPanel/useTunnelList"

const { translate, toast } = vi.hoisted(() => ({ translate: (key: string) => key, toast: vi.fn() }))
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: translate }) }))
vi.mock("@/hooks/use-toast", () => ({ useToast: () => ({ toast }) }))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }))

it("loads the tunnel snapshot even when live status registration fails", async () => {
  vi.mocked(listen).mockRejectedValue(new Error("subscription failed"))
  vi.mocked(invoke).mockResolvedValue([])
  const { result, unmount } = renderHook(() => useTunnelList(undefined))
  await waitFor(() => expect(result.current.loading).toBe(false))
  expect(invoke).toHaveBeenCalledWith("list_tunnels")
  expect(invoke).toHaveBeenCalledWith("list_tunnel_statuses")
  expect(toast).toHaveBeenCalledWith(
    expect.objectContaining({ title: "tunnels.liveStatusUnavailable" })
  )
  unmount()
})
