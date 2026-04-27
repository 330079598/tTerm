import * as React from "react"
import { AlertTriangle, Info } from "lucide-react"

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
import { Label } from "@/components/ui/label"

interface ConfirmDialogOptions {
  title: React.ReactNode
  description?: React.ReactNode
  confirmText?: React.ReactNode
  cancelText?: React.ReactNode
  variant?: "default" | "destructive"
}

type ConfirmDialogState = ConfirmDialogOptions & {
  resolve: (confirmed: boolean) => void
}

interface PromptDialogOptions {
  title: React.ReactNode
  label?: React.ReactNode
  description?: React.ReactNode
  defaultValue?: string
  placeholder?: string
  confirmText?: React.ReactNode
  cancelText?: React.ReactNode
}

type PromptDialogState = PromptDialogOptions & {
  resolve: (value: string | null) => void
}

interface InfoDialogOptions {
  title: React.ReactNode
  description?: React.ReactNode
  closeText?: React.ReactNode
}

type InfoDialogState = InfoDialogOptions & {
  resolve: () => void
}

export function useConfirmDialog() {
  const [state, setState] = React.useState<ConfirmDialogState | null>(null)

  const confirm = React.useCallback((options: ConfirmDialogOptions) => {
    return new Promise<boolean>((resolve) => {
      setState({ ...options, resolve })
    })
  }, [])

  const close = React.useCallback(
    (confirmed: boolean) => {
      state?.resolve(confirmed)
      setState(null)
    },
    [state]
  )

  const ConfirmDialog = React.useCallback(() => {
    if (!state) {
      return null
    }

    const isDestructive = state.variant === "destructive"

    return (
      <Dialog open onOpenChange={(open) => !open && close(false)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              {isDestructive && <AlertTriangle className="text-destructive size-4" />}
              {state.title}
            </DialogTitle>
            {state.description && (
              <DialogDescription className="whitespace-pre-line">
                {state.description}
              </DialogDescription>
            )}
          </DialogHeader>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => close(false)}>
              {state.cancelText ?? "Cancel"}
            </Button>
            <Button
              type="button"
              variant={isDestructive ? "destructive" : "default"}
              onClick={() => close(true)}
            >
              {state.confirmText ?? "Confirm"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    )
  }, [close, state])

  return { confirm, ConfirmDialog }
}

export function usePromptDialog() {
  const [state, setState] = React.useState<PromptDialogState | null>(null)
  const [value, setValue] = React.useState("")

  const prompt = React.useCallback((options: PromptDialogOptions) => {
    return new Promise<string | null>((resolve) => {
      setValue(options.defaultValue ?? "")
      setState({ ...options, resolve })
    })
  }, [])

  const close = React.useCallback(
    (result: string | null) => {
      state?.resolve(result)
      setState(null)
    },
    [state]
  )

  const PromptDialog = React.useCallback(() => {
    if (!state) {
      return null
    }

    return (
      <Dialog open onOpenChange={(open) => !open && close(null)}>
        <DialogContent className="sm:max-w-md">
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault()
              close(value)
            }}
          >
            <DialogHeader>
              <DialogTitle>{state.title}</DialogTitle>
              {state.description && (
                <DialogDescription className="whitespace-pre-line">
                  {state.description}
                </DialogDescription>
              )}
            </DialogHeader>
            <div className="space-y-2">
              {state.label && <Label htmlFor="app-prompt-input">{state.label}</Label>}
              <Input
                id="app-prompt-input"
                autoFocus
                value={value}
                placeholder={state.placeholder}
                onChange={(event) => setValue(event.target.value)}
              />
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => close(null)}>
                {state.cancelText ?? "Cancel"}
              </Button>
              <Button type="submit">{state.confirmText ?? "OK"}</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    )
  }, [close, state, value])

  return { prompt, PromptDialog }
}

export function useInfoDialog() {
  const [state, setState] = React.useState<InfoDialogState | null>(null)

  const info = React.useCallback((options: InfoDialogOptions) => {
    return new Promise<void>((resolve) => {
      setState({ ...options, resolve })
    })
  }, [])

  const close = React.useCallback(() => {
    state?.resolve()
    setState(null)
  }, [state])

  const InfoDialog = React.useCallback(() => {
    if (!state) {
      return null
    }

    return (
      <Dialog open onOpenChange={(open) => !open && close()}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Info className="text-primary size-4" />
              {state.title}
            </DialogTitle>
            {state.description && (
              <DialogDescription className="whitespace-pre-line">
                {state.description}
              </DialogDescription>
            )}
          </DialogHeader>
          <DialogFooter>
            <Button type="button" onClick={close}>
              {state.closeText ?? "OK"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    )
  }, [close, state])

  return { info, InfoDialog }
}
