import * as React from "react"

import type { ToastActionElement, ToastProps } from "@/components/ui/toast"

const TOAST_LIMIT = 1
const TOAST_DURATION = 3000 // 3 seconds

type ToasterToast = ToastProps & {
  id: string
  title?: React.ReactNode
  description?: React.ReactNode
  action?: ToastActionElement
}

let count = 0

function genId() {
  count = (count + 1) % Number.MAX_SAFE_INTEGER
  return count.toString()
}

type Action =
  | {
      type: "ADD_TOAST"
      toast: ToasterToast
    }
  | {
      type: "UPDATE_TOAST"
      toast: Partial<ToasterToast>
    }
  | {
      type: "DISMISS_TOAST"
      toastId?: ToasterToast["id"]
    }
  | {
      type: "REMOVE_TOAST"
      toastId?: ToasterToast["id"]
    }

interface State {
  toasts: ToasterToast[]
}

const toastTimeouts = new Map<string, ReturnType<typeof setTimeout>>()

const clearToastTimeout = (toastId: string) => {
  const timeout = toastTimeouts.get(toastId)
  if (!timeout) {
    return
  }

  clearTimeout(timeout)
  toastTimeouts.delete(toastId)
}

const scheduleToastRemoval = (toastId: string, duration: number) => {
  clearToastTimeout(toastId)

  if (!Number.isFinite(duration) || duration <= 0) {
    return
  }

  const timeout = setTimeout(() => {
    toastTimeouts.delete(toastId)
    dispatch({ type: "REMOVE_TOAST", toastId })
  }, duration)

  toastTimeouts.set(toastId, timeout)
}

export const reducer = (state: State, action: Action): State => {
  switch (action.type) {
    case "ADD_TOAST": {
      const nextToasts = [action.toast, ...state.toasts].slice(0, TOAST_LIMIT)
      const removedToasts = state.toasts.filter(
        (toast) => !nextToasts.some((nextToast) => nextToast.id === toast.id)
      )

      removedToasts.forEach((toast) => {
        clearToastTimeout(toast.id)
      })
      scheduleToastRemoval(action.toast.id, action.toast.duration ?? TOAST_DURATION)

      return {
        ...state,
        toasts: nextToasts,
      }
    }

    case "UPDATE_TOAST": {
      const nextToasts = state.toasts.map((t) =>
        t.id === action.toast.id ? { ...t, ...action.toast } : t
      )
      const updatedToast = nextToasts.find((t) => t.id === action.toast.id)

      if (updatedToast) {
        scheduleToastRemoval(updatedToast.id, updatedToast.duration ?? TOAST_DURATION)
      }

      return {
        ...state,
        toasts: nextToasts,
      }
    }

    case "DISMISS_TOAST":
    case "REMOVE_TOAST": {
      const { toastId } = action

      if (toastId === undefined) {
        state.toasts.forEach((toast) => {
          clearToastTimeout(toast.id)
        })
        return {
          ...state,
          toasts: [],
        }
      }

      clearToastTimeout(toastId)
      return {
        ...state,
        toasts: state.toasts.filter((t) => t.id !== toastId),
      }
    }
  }
}

const listeners: Array<(state: State) => void> = []

let memoryState: State = { toasts: [] }

function dispatch(action: Action) {
  memoryState = reducer(memoryState, action)
  listeners.forEach((listener) => {
    listener(memoryState)
  })
}

type Toast = Omit<ToasterToast, "id">

function toast({ ...props }: Toast) {
  const id = genId()
  const { duration = TOAST_DURATION, open: _open, ...restProps } = props

  const update = (props: ToasterToast) =>
    dispatch({
      type: "UPDATE_TOAST",
      toast: { ...props, id },
    })
  const dismiss = () => dispatch({ type: "DISMISS_TOAST", toastId: id })

  dispatch({
    type: "ADD_TOAST",
    toast: {
      ...restProps,
      id,
      duration,
    },
  })

  return {
    id: id,
    dismiss,
    update,
  }
}

function useToast() {
  const [state, setState] = React.useState<State>(memoryState)

  React.useEffect(() => {
    listeners.push(setState)
    return () => {
      const index = listeners.indexOf(setState)
      if (index > -1) {
        listeners.splice(index, 1)
      }
    }
  }, [])

  return {
    ...state,
    toast,
    dismiss: (toastId?: string) => dispatch({ type: "DISMISS_TOAST", toastId }),
  }
}

export { useToast, toast }
