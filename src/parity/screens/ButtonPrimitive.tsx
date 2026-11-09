import { Button } from "@/components/ui/button"
import type { ParityScreenProps } from "@/parity/types"
import { useEffect } from "react"
const variants = ["default", "destructive", "outline", "secondary", "ghost", "link"] as const
const sizes = ["default", "xs", "sm", "lg", "icon", "icon-xs", "icon-sm", "icon-lg"] as const
export function ButtonVariants({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main
      data-parity="ui.button.variants.screen"
      className="bg-background text-foreground h-full w-full p-8 text-sm"
    >
      <div className="flex flex-col gap-3">
        {variants.map((variant) => (
          <Button key={variant} data-parity={`ui.button.variants.${variant}`} variant={variant}>
            {variant}
          </Button>
        ))}
        <Button
          data-parity="ui.button.variants.hover"
          variant="default"
          className="hover:bg-primary"
        >
          hover
        </Button>
        <Button data-parity="ui.button.variants.focus" variant="default" autoFocus>
          focus
        </Button>
        <Button data-parity="ui.button.variants.disabled" variant="default" disabled>
          disabled
        </Button>
        <Button
          data-parity="ui.button.variants.disabled-destructive"
          variant="destructive"
          disabled
        >
          disabled destructive
        </Button>
        <Button data-parity="ui.button.variants.disabled-outline" variant="outline" disabled>
          disabled outline
        </Button>
      </div>
    </main>
  )
}
export function ButtonSizes({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main
      data-parity="ui.button.sizes.screen"
      className="bg-background text-foreground h-full w-full p-8 text-sm"
    >
      <div className="flex flex-col gap-3">
        {sizes.map((size) => (
          <Button key={size} data-parity={`ui.button.sizes.${size}`} size={size}>
            {size.startsWith("icon") || size === "icon" ? "＋" : size}
          </Button>
        ))}
      </div>
    </main>
  )
}
