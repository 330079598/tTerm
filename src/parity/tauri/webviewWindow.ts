// Replaces `@tauri-apps/api/webviewWindow` when Vite runs with `--mode parity`.
// The webview background is set natively; the reference capture has no native
// window to paint, so the only method the theme code calls is a no-op. Other
// window methods still fail, because a screen that needs them cannot be
// captured faithfully in a browser.

export function getCurrentWebviewWindow() {
  return {
    setBackgroundColor: async (color: number[]): Promise<void> => {
      void color
    },
  }
}
