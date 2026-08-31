import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Error message extraction
export function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

// HSL color helper
export function hslToCssColor(hsl: string): string {
  return `hsl(${hsl})`
}
