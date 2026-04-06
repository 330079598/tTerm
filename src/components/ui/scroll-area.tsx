"use client"

import * as React from "react"

import { cn } from "@/lib/utils"

function ScrollArea({ className, children, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div data-slot="scroll-area" className={cn("relative overflow-auto", className)} {...props}>
      <div data-slot="scroll-area-viewport" className="size-full rounded-[inherit]">
        {children}
      </div>
    </div>
  )
}

function ScrollBar({
  className,
  orientation = "vertical",
  ...props
}: React.HTMLAttributes<HTMLDivElement> & {
  orientation?: "horizontal" | "vertical"
}) {
  return (
    <div
      data-slot="scroll-area-scrollbar"
      data-orientation={orientation}
      aria-hidden="true"
      className={cn(
        "pointer-events-none absolute transition-colors select-none",
        orientation === "vertical" && "inset-y-0 right-0 w-2.5 border-l border-l-transparent p-px",
        orientation === "horizontal" &&
          "right-0 bottom-0 left-0 h-2.5 border-t border-t-transparent p-px",
        className
      )}
      {...props}
    >
      <div
        data-slot="scroll-area-thumb"
        className="bg-border relative h-full w-full rounded-full opacity-40"
      />
    </div>
  )
}

export { ScrollArea, ScrollBar }
