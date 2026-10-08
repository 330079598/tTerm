/**
 * Whether a keydown belongs to the input method rather than the page.
 *
 * Chromium flags keys pressed during a composition with `isComposing`, but
 * WebKit (macOS) fires the Enter or Escape that ends a composition after
 * compositionend, with `isComposing` false. Both report keyCode 229 for a key
 * the IME consumed, so handlers that act on Enter, Escape or arrows in a text
 * field must check this first.
 */
export function isImeKeyEvent(event: Pick<KeyboardEvent, "isComposing" | "keyCode">): boolean {
  return event.isComposing || event.keyCode === 229
}
