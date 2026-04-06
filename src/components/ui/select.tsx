import * as React from "react"
import { ChevronDown } from "lucide-react"

import { cn } from "@/lib/utils"

function Select({ className, children, ...props }: React.ComponentProps<"select">) {
  return (
    <div className="relative">
      <select
        data-slot="select"
        className={cn(
          "border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full appearance-none rounded-md border px-3 py-1 pr-9 text-sm shadow-xs transition-[color,box-shadow] outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50",
          className
        )}
        {...props}
      >
        {children}
      </select>
      <ChevronDown className="text-muted-foreground pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2" />
    </div>
  )
}

export { Select }
