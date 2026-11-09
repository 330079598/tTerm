import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import type { ParityScreenProps } from "@/parity/types"
import { useEffect } from "react"

export function DialogConfirmPrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground flex h-full w-full items-center justify-center p-8">
      <Dialog open>
        <DialogContent data-parity="ui.dialog.confirm.content">
          <DialogHeader data-parity="ui.dialog.confirm.header">
            <DialogTitle data-parity="ui.dialog.confirm.title">Delete profile?</DialogTitle>
            <DialogDescription data-parity="ui.dialog.confirm.description">
              This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter data-parity="ui.dialog.confirm.footer">
            <Button data-parity="ui.dialog.confirm.cancel" variant="outline">
              Cancel
            </Button>
            <Button data-parity="ui.dialog.confirm.confirm">Confirm</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </main>
  )
}

export function DialogDestructivePrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground flex h-full w-full items-center justify-center p-8">
      <Dialog open>
        <DialogContent data-parity="ui.dialog.destructive.content">
          <DialogHeader>
            <DialogTitle data-parity="ui.dialog.destructive.title">Remove connection?</DialogTitle>
            <DialogDescription>All saved credentials will be removed.</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline">Cancel</Button>
            <Button variant="destructive" data-parity="ui.dialog.destructive.confirm">
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </main>
  )
}

export function DialogPromptPrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground flex h-full w-full items-center justify-center p-8">
      <Dialog open>
        <DialogContent data-parity="ui.dialog.prompt.content">
          <form className="space-y-4">
            <DialogHeader>
              <DialogTitle data-parity="ui.dialog.prompt.title">Rename profile</DialogTitle>
              <DialogDescription>Choose a new name for this profile.</DialogDescription>
            </DialogHeader>
            <Input data-parity="ui.dialog.prompt.input" autoFocus defaultValue="Production" />
            <DialogFooter>
              <Button variant="outline">Cancel</Button>
              <Button>OK</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </main>
  )
}
