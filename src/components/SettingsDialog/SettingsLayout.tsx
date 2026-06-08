import React from "react"

import { Card, CardContent } from "@/components/ui/card"
import { cn } from "@/lib/utils"

interface SettingsSectionProps {
  children: React.ReactNode
  description?: React.ReactNode
  icon?: React.ReactNode
  title: React.ReactNode
}

export const SettingsSection: React.FC<SettingsSectionProps> = ({
  children,
  description,
  icon,
  title,
}) => {
  return (
    <section className="space-y-3">
      <div className="flex items-start gap-2">
        {icon && <span className="text-muted-foreground mt-0.5 shrink-0">{icon}</span>}
        <div>
          <h3 className="text-sm font-semibold">{title}</h3>
          {description && (
            <p className="text-muted-foreground mt-1 text-xs leading-5">{description}</p>
          )}
        </div>
      </div>
      <div className="space-y-2">{children}</div>
    </section>
  )
}

interface SettingsRowProps {
  action?: React.ReactNode
  children?: React.ReactNode
  className?: string
  description?: React.ReactNode
  icon?: React.ReactNode
  title: React.ReactNode
}

export const SettingsRow: React.FC<SettingsRowProps> = ({
  action,
  children,
  className,
  description,
  icon,
  title,
}) => {
  return (
    <Card className={cn("overflow-hidden border-transparent shadow-none", className)}>
      <CardContent className="p-4">
        <div className="flex items-center justify-between gap-4">
          <div className="flex min-w-0 items-start gap-3">
            {icon && <span className="mt-0.5 shrink-0">{icon}</span>}
            <div className="min-w-0">
              <div className="text-sm font-medium">{title}</div>
              {description && (
                <div className="text-muted-foreground mt-1 text-xs leading-5">{description}</div>
              )}
            </div>
          </div>
          {action && <div className="shrink-0">{action}</div>}
        </div>
        {children && <div className="mt-4">{children}</div>}
      </CardContent>
    </Card>
  )
}
