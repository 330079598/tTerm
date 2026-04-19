import React from "react"
import { Check } from "lucide-react"

import { ThemePreviewSwatches } from "@/components/ThemePreviewSwatches"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { cn, hslToCssColor } from "@/lib/utils"
import type { Theme } from "@/types/theme"

interface ThemeCardProps {
  actionSlot?: React.ReactNode
  compactPreview?: boolean
  currentTheme: string
  description: string
  name: string
  onSelect: () => void
  theme: Theme
}

export const ThemeCard: React.FC<ThemeCardProps> = ({
  actionSlot,
  compactPreview = false,
  currentTheme,
  description,
  name,
  onSelect,
  theme,
}) => {
  const isActive = currentTheme === theme.id

  return (
    <Card className="overflow-hidden border-transparent shadow-none">
      <CardContent className="p-0">
        <div className="flex items-center">
          <Button
            type="button"
            variant="ghost"
            onClick={onSelect}
            className={cn(
              "h-auto flex-1 justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
              isActive ? "border-primary bg-accent" : "border-transparent"
            )}
          >
            <span
              className="border-border size-5 shrink-0 rounded-full border"
              style={{ background: hslToCssColor(theme.colors.background) }}
            />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-3">
                <div className="flex min-w-0 flex-1 flex-col items-start">
                  <span className="truncate text-sm leading-none font-medium">{name}</span>
                  <span className="text-muted-foreground mt-1 truncate text-xs">{description}</span>
                </div>
                <ThemePreviewSwatches compact={compactPreview} palette={theme.terminal} />
              </div>
            </div>
            {isActive && <Check size={16} className="text-primary ml-2 shrink-0" />}
          </Button>
          {actionSlot}
        </div>
      </CardContent>
    </Card>
  )
}
