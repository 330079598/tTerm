import { useConfirmDialog, usePromptDialog } from "@/components/ui/app-dialog"
import type { ParityScreenProps } from "@/parity/types"
import { useEffect } from "react"

/** Marks the production hook's portal without introducing fixture CSS. */
function markDialog(prefix: string, onReady: () => void) {
  const content = document.querySelector('[data-slot="dialog-content"]')
  if (!content) return
  content.setAttribute("data-parity", `${prefix}.content`)
  document
    .querySelector('[data-slot="dialog-overlay"]')
    ?.setAttribute("data-parity", `${prefix}.overlay`)
  const buttons = content.querySelectorAll('[data-slot="dialog-footer"] button')
  buttons[0]?.setAttribute("data-parity", `${prefix}.cancel`)
  buttons[1]?.setAttribute("data-parity", `${prefix}.confirm`)
  onReady()
}

function Confirm({ onReady, destructive }: ParityScreenProps & { destructive: boolean }) {
  const { confirm, ConfirmDialog } = useConfirmDialog()
  useEffect(() => {
    void confirm({
      title: destructive ? "Remove connection?" : "Delete profile?",
      description: destructive
        ? "All saved credentials will be removed."
        : "This action cannot be undone.",
      confirmText: destructive ? "Delete" : "Confirm",
      cancelText: "Cancel",
      variant: destructive ? "destructive" : "default",
    })
  }, [confirm, destructive])
  useEffect(() => {
    markDialog(destructive ? "ui.dialog.destructive" : "ui.dialog.confirm", onReady)
  }, [ConfirmDialog, destructive, onReady])
  return (
    <main className="bg-background text-foreground h-full w-full">
      <ConfirmDialog />
    </main>
  )
}

export function DialogConfirmPrimitive(props: ParityScreenProps) {
  return <Confirm {...props} destructive={false} />
}

export function DialogDestructivePrimitive(props: ParityScreenProps) {
  return <Confirm {...props} destructive />
}

export function DialogPromptPrimitive({ onReady }: ParityScreenProps) {
  const { prompt, PromptDialog } = usePromptDialog()
  useEffect(() => {
    void prompt({
      title: "Rename profile",
      description: "Choose a new name for this profile.",
      defaultValue: "Production",
      cancelText: "Cancel",
      confirmText: "OK",
    })
  }, [prompt])
  useEffect(() => markDialog("ui.dialog.prompt", onReady), [PromptDialog, onReady])
  return (
    <main className="bg-background text-foreground h-full w-full">
      <PromptDialog />
    </main>
  )
}
