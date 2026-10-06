import { invoke } from "@tauri-apps/api/core"

/** Releases what the backend keeps for a closed tab, such as passwords typed while connecting. */
function cleanupConnection(tabId: string) {
  invoke("forget_typed_passwords", { tabId }).catch(console.error)
}

export function useConnectionManager() {
  return { cleanupConnection }
}
