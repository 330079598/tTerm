import { useEffect } from "react"
import { ContextMenu } from "@/components/ContextMenu"
import { ConfigProvider } from "@/contexts/ConfigContext"
import { KeymapProvider } from "@/contexts/KeymapContext"
import { pageTabMenu } from "@/parity/screens/contextMenuFixture"
import type { ParityScreenProps } from "@/parity/types"

function Menu({ onReady, highlighted }: ParityScreenProps & { highlighted: boolean }) {
  useEffect(() => {
    if (highlighted)
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }))
    onReady()
  }, [onReady, highlighted])
  return (
    <main className="bg-background text-foreground h-full w-full">
      <ContextMenu
        data-parity="ui.context-menu"
        x={760}
        y={550}
        actions={pageTabMenu()}
        onAction={() => {}}
        onClose={() => {}}
      />
    </main>
  )
}

function Screen(props: ParityScreenProps & { highlighted: boolean }) {
  return (
    <ConfigProvider>
      <KeymapProvider>
        <Menu {...props} />
      </KeymapProvider>
    </ConfigProvider>
  )
}
export function ContextMenuDefault(props: ParityScreenProps) {
  return <Screen {...props} highlighted={false} />
}
export function ContextMenuHighlighted(props: ParityScreenProps) {
  return <Screen {...props} highlighted />
}
