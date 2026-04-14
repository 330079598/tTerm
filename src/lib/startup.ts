const STARTUP_STATUS = {
  configReady: false,
  themeReady: false,
  sessionReady: false,
  splashHidden: false,
}

const APP_READY_EVENT = "tterm:app-ready"

function dispatchReadyIfComplete(): void {
  if (
    STARTUP_STATUS.configReady &&
    STARTUP_STATUS.themeReady &&
    STARTUP_STATUS.sessionReady &&
    !STARTUP_STATUS.splashHidden
  ) {
    window.dispatchEvent(new CustomEvent(APP_READY_EVENT))
  }
}

export function markConfigReady(): void {
  STARTUP_STATUS.configReady = true
  dispatchReadyIfComplete()
}

export function announceThemeReady(): void {
  STARTUP_STATUS.themeReady = true
  dispatchReadyIfComplete()
}

export function markSessionReady(): void {
  STARTUP_STATUS.sessionReady = true
  dispatchReadyIfComplete()
}

export function onAppReady(callback: () => void): () => void {
  const handler = () => {
    if (STARTUP_STATUS.splashHidden) {
      return
    }

    STARTUP_STATUS.splashHidden = true
    callback()
  }

  window.addEventListener(APP_READY_EVENT, handler)
  dispatchReadyIfComplete()

  return () => {
    window.removeEventListener(APP_READY_EVENT, handler)
  }
}
