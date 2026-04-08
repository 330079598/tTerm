import "react"

declare module "react" {
  interface HTMLAttributes<T> extends AriaAttributes, DOMAttributes<T> {
    "data-tauri-drag-region"?: boolean
  }
}

// Tauri extended File interface with path property
interface TauriFile extends File {
  path: string
}

declare global {
  interface File {
    path?: string
  }
}
