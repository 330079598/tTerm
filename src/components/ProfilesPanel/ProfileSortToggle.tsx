import React from "react"
import { useTranslation } from "react-i18next"

import type { ProfileSort } from "@/lib/appConfig"
import { cn } from "@/lib/utils"

const SORTS: readonly ProfileSort[] = ["default", "recent"]

/** Switches the connections panel between its groups and one list by last use. */
export const ProfileSortToggle: React.FC<{
  sort: ProfileSort
  onSortChange: (sort: ProfileSort) => void
}> = ({ sort, onSortChange }) => {
  const { t } = useTranslation()

  return (
    <div
      className="bg-muted/55 grid shrink-0 grid-cols-2 rounded-md p-1"
      role="group"
      aria-label={t("profiles.sort.label")}
    >
      {SORTS.map((value) => (
        <button
          key={value}
          type="button"
          aria-pressed={sort === value}
          onClick={() => onSortChange(value)}
          className={cn(
            "focus-visible:ring-ring/50 rounded-sm px-3 text-xs font-medium whitespace-nowrap transition-colors outline-none focus-visible:ring-[3px]",
            sort === value
              ? "bg-background text-foreground shadow-xs"
              : "text-muted-foreground hover:text-foreground"
          )}
        >
          {t(`profiles.sort.${value}`)}
        </button>
      ))}
    </div>
  )
}
