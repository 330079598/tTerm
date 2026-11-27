import { useCallback, useEffect, useRef, useState } from "react"

import { ProfilesPanel } from "@/components/ProfilesPanel"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { ConfigProvider } from "@/contexts/ConfigContext"
import { ThemeProvider } from "@/contexts/ThemeContext"
import type { ParityScreenProps } from "@/parity/types"
import { firstScreenProfiles } from "@/parity/screens/firstScreen/fixture"
import { FirstScreenTitleBar } from "@/parity/screens/firstScreen/TitleBar"
import { FirstScreenTerminal } from "@/parity/screens/firstScreen/TerminalSurface"

/** Both ready barriers: the production chrome's config and the synthetic surface. */
function useFirstScreenReady(onReady: () => void, parts: number) {
  const [ready, setReady] = useState<boolean[]>(() => Array(parts).fill(false))
  const fired = useRef(false)
  const mark = useCallback((index: number) => {
    setReady((current) => {
      if (current[index]) return current
      const next = [...current]
      next[index] = true
      return next
    })
  }, [])
  useEffect(() => {
    if (!fired.current && ready.every(Boolean)) {
      fired.current = true
      onReady()
    }
  }, [onReady, ready])
  return mark
}

export function WorkspaceFirstScreen({ onReady }: ParityScreenProps) {
  return (
    <ConfigProvider>
      <ThemeProvider>
        <WorkspaceContent onReady={onReady} />
      </ThemeProvider>
    </ConfigProvider>
  )
}

function WorkspaceContent({ onReady }: ParityScreenProps) {
  const markReady = useFirstScreenReady(onReady, 2)
  const chromeReady = useCallback(() => markReady(0), [markReady])
  const surfaceReady = useCallback(() => markReady(1), [markReady])
  return (
    <FirstScreenTitleBar onReady={chromeReady}>
      <FirstScreenTerminal onReady={surfaceReady} />
    </FirstScreenTitleBar>
  )
}

export function ProfilesFirstScreen({ onReady }: ParityScreenProps) {
  return (
    <ConfigProvider>
      <ThemeProvider>
        <ProfilesContent onReady={onReady} />
      </ThemeProvider>
    </ConfigProvider>
  )
}

function ProfilesContent({ onReady }: ParityScreenProps) {
  const markReady = useFirstScreenReady(onReady, 3)
  const chromeReady = useCallback(() => markReady(0), [markReady])
  const surfaceReady = useCallback(() => markReady(1), [markReady])
  const dialogReady = useCallback(() => markReady(2), [markReady])
  return (
    <>
      <FirstScreenTitleBar onReady={chromeReady}>
        <FirstScreenTerminal onReady={surfaceReady} />
      </FirstScreenTitleBar>
      <ProfileDialog onReady={dialogReady} />
    </>
  )
}

function ProfileDialog({ onReady }: { onReady: () => void }) {
  const fired = useRef(false)
  useEffect(() => {
    let frame = 0
    const check = () => {
      if (fired.current) return
      const loaded =
        document.querySelector("[data-slot='dialog-content'] [role='option']") !== null &&
        document.querySelectorAll("[data-slot='dialog-content'] [role='option']").length ===
          firstScreenProfiles.length
      if (!loaded) {
        frame = requestAnimationFrame(check)
        return
      }
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          if (!fired.current) {
            fired.current = true
            onReady()
          }
        })
      )
    }
    frame = requestAnimationFrame(check)
    return () => cancelAnimationFrame(frame)
  }, [onReady])

  return (
    <Dialog open onOpenChange={() => {}}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(720px,85vh)] flex-col overflow-hidden border-0 p-0 shadow-none sm:max-w-3xl"
      >
        <ProfilesPanel
          refreshKey={0}
          collapsedGroupKeys={[]}
          profileRecents={[]}
          surface="panel"
          onCollapsedGroupKeysChange={() => {}}
          onClose={() => {}}
          onCreate={() => {}}
          onConnect={() => {}}
          onEdit={() => {}}
          onDuplicate={() => {}}
          onDeleteProfile={() => {}}
        />
      </DialogContent>
    </Dialog>
  )
}
