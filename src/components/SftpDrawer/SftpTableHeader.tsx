import { ArrowDown, ArrowUp } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { SftpDrawerContentProps } from "@/components/SftpDrawer/SftpDrawerContent"
import {
  SFTP_COLUMN_DEFAULT_WEIGHTS,
  type useSftpColumnWidths,
} from "@/components/SftpDrawer/useSftpColumnWidths"
import { cn } from "@/lib/utils"
import { SFTP_SORT_COLUMNS } from "@/components/SftpDrawer/sftpView"

type SftpTableHeaderProps = Pick<
  SftpDrawerContentProps,
  "isSelectionMode" | "sort" | "onSortColumn"
> &
  Pick<
    ReturnType<typeof useSftpColumnWidths>,
    "tableGridStyle" | "columnWeights" | "startColumnResize" | "adjustColumnWidth"
  > & {
    /** Shown in the last column's place while filtering or selecting. */
    resultSummary: string | null
  }

/** The listing's column titles: each sorts by its column, and its edge resizes it. */
export function SftpTableHeader({
  isSelectionMode,
  tableGridStyle,
  resultSummary,
  sort,
  onSortColumn,
  columnWeights,
  startColumnResize,
  adjustColumnWidth,
}: SftpTableHeaderProps) {
  const { t } = useTranslation()

  return (
    <div
      className={cn("sftp-table-header", isSelectionMode && "sftp-table-header-selection")}
      role="row"
      style={tableGridStyle}
    >
      {isSelectionMode && <span className="sftp-header-cell" aria-hidden="true" />}
      {[
        t("sftp.columns.name"),
        t("sftp.columns.modified"),
        t("sftp.columns.size"),
        t("sftp.columns.kind", { defaultValue: "Kind" }),
        t("sftp.columns.permissions", { defaultValue: "Permissions" }),
        resultSummary ?? t("sftp.columns.owner"),
      ].map((label, index) => {
        const column = SFTP_SORT_COLUMNS[index]
        const isLast = index === SFTP_COLUMN_DEFAULT_WEIGHTS.length - 1
        // The last cell doubles as the result summary, which is not a column title.
        const sortable = !(isLast && resultSummary)
        const sorted = sortable && sort.column === column
        return (
          <span
            key={index}
            className={cn("sftp-header-cell", isLast && "sftp-header-summary")}
            aria-live={isLast ? "polite" : undefined}
            aria-sort={sorted ? (sort.direction === "asc" ? "ascending" : "descending") : undefined}
          >
            {sortable ? (
              <button
                type="button"
                className="sftp-header-sort"
                onClick={() => onSortColumn(column)}
                title={t("sftp.columns.sortBy", {
                  column: label,
                  defaultValue: `Sort by ${label}`,
                })}
              >
                <span className="truncate">{label}</span>
                {sorted &&
                  (sort.direction === "asc" ? (
                    <ArrowUp className="size-3 shrink-0" aria-hidden="true" />
                  ) : (
                    <ArrowDown className="size-3 shrink-0" aria-hidden="true" />
                  ))}
              </button>
            ) : (
              label
            )}
            {index < SFTP_COLUMN_DEFAULT_WEIGHTS.length - 1 && (
              <span
                className="sftp-column-resizer"
                role="separator"
                tabIndex={0}
                aria-orientation="vertical"
                aria-valuenow={Math.round(columnWeights[index])}
                aria-label={t("sftp.columns.resize", {
                  column: label,
                  defaultValue: `Resize ${label}`,
                })}
                onMouseDown={(event) => startColumnResize(index, event)}
                onKeyDown={(event) => {
                  if (event.key === "ArrowLeft") {
                    event.preventDefault()
                    adjustColumnWidth(index, -12)
                  } else if (event.key === "ArrowRight") {
                    event.preventDefault()
                    adjustColumnWidth(index, 12)
                  }
                }}
              />
            )}
          </span>
        )
      })}
    </div>
  )
}
