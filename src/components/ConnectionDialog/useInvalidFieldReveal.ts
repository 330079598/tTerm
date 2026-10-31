import type { TFunction } from "i18next"
import type React from "react"
import { useRef } from "react"

import type { useToast } from "@/hooks/use-toast"

type FormField = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement

const isFormField = (target: EventTarget | null): target is FormField =>
  target instanceof HTMLInputElement ||
  target instanceof HTMLSelectElement ||
  target instanceof HTMLTextAreaElement

/**
 * Scrolls a field to the middle of the dialog and focuses it, so the user
 * sees what stopped the submit even when it sits far above the buttons.
 */
export function revealField(element: HTMLElement | null) {
  if (!element) return
  element.scrollIntoView?.({ block: "center" })
  element.focus({ preventScroll: true })
}

/** Reveals the field with this id once React has rendered its error. */
export function revealFieldById(id: string) {
  window.setTimeout(() => revealField(document.getElementById(id)))
}

/**
 * Says what is wrong with a field the form's constraints rejected, in the
 * app's language rather than the webview's.
 */
export function describeInvalidField(field: FormField, t: TFunction): string {
  const name = field.labels?.[0]?.textContent?.trim() || field.name || field.id
  const { validity } = field
  if (validity.valueMissing) return t("connection.validation.required", { field: name })
  if (validity.badInput) return t("connection.validation.notNumber", { field: name })
  if (validity.rangeUnderflow || validity.rangeOverflow) {
    const { min, max } = field as HTMLInputElement
    return max
      ? t("connection.validation.between", { field: name, min, max })
      : t("connection.validation.atLeast", { field: name, min })
  }
  return field.validationMessage ? `${name}: ${field.validationMessage}` : name
}

interface UseInvalidFieldRevealOptions {
  toast: ReturnType<typeof useToast>["toast"]
  t: TFunction
}

/**
 * Handles the `invalid` events a blocked submit fires, for the form's
 * `onInvalidCapture`. The webview does not always show the browser's own
 * bubble, which left a click on Connect looking like it did nothing, so the
 * first invalid field is scrolled to, focused and named in a toast instead.
 */
export function useInvalidFieldReveal({ toast, t }: UseInvalidFieldRevealOptions) {
  // One submit fires an `invalid` event for every bad field, in document
  // order; only the first one is reported.
  const reporting = useRef(false)

  return (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (reporting.current || !isFormField(event.target)) return
    reporting.current = true
    window.setTimeout(() => {
      reporting.current = false
    })
    const field = event.target
    revealField(field)
    toast({
      title: t("connection.validation.title"),
      description: describeInvalidField(field, t),
      variant: "destructive",
    })
  }
}
