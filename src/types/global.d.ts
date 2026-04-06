import "react"

declare module "react" {
  interface HTMLAttributes<T> extends AriaAttributes, DOMAttributes<T> {
    "data-tauri-drag-region"?: boolean
  }
}

// Tauri 扩展的 File 接口，包含 path 属性
interface TauriFile extends File {
  path: string
}

declare global {
  interface File {
    path?: string
  }
}
