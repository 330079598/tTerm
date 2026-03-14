export function useConnectionManager() {
  return {
    cleanupConnection: (_tabId: string) => {
      // No-op for now; reserved for future connection cleanup logic
    },
  }
}
