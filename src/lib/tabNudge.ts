/**
 * Brief tab-bar animations for tab shortcuts that leave the active tab as it
 * is, so a shortcut that changed nothing still shows it was received.
 */

const TAB_FLASH_CLASS = "tab-nudge-flash"
const TAB_BAR_SHAKE_CLASS = "tab-nudge-shake"

// Elements that already listen for the end of their nudge, so replaying a
// nudge mid-animation doesn't stack another listener.
const pendingElements = new WeakSet<HTMLElement>()

// Removing the class and forcing a reflow before adding it back restarts the
// animation, so pressing the shortcut again replays it.
function replayNudge(element: HTMLElement, className: string) {
  element.classList.remove(className)
  void element.offsetWidth
  element.classList.add(className)
  if (pendingElements.has(element)) return
  pendingElements.add(element)

  // A tab's agent spinner and ripple end inside it too, and their events
  // bubble up here, so only the element's own animation ends the nudge.
  const handleEnd = (event: Event) => {
    if (event.target !== element) return
    element.classList.remove(className)
    element.removeEventListener("animationend", handleEnd)
    pendingElements.delete(element)
  }
  element.addEventListener("animationend", handleEnd)
}

/** Flashes a tab in the tab bar, for a shortcut whose target is the tab already active. */
export function nudgeTab(tabId: string) {
  const tab = Array.from(document.querySelectorAll<HTMLElement>(".tab-item")).find(
    (candidate) => candidate.dataset.tabId === tabId
  )
  if (!tab) return
  // The active tab may have been scrolled out of the bar since it was chosen.
  tab.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "nearest" })
  replayNudge(tab, TAB_FLASH_CLASS)
}

/** Shakes the tab bar, for a shortcut that names no tab, such as Cmd+5 with three tabs open. */
export function nudgeTabBar() {
  const bar = document.querySelector<HTMLElement>(".tab-list-viewport")
  if (bar) replayNudge(bar, TAB_BAR_SHAKE_CLASS)
}
