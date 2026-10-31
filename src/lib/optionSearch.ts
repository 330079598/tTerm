import { matchesQuery } from "@/lib/quickConnect"

/** One choice of a searchable list. */
export interface SearchableOption {
  value: string
  label: string
  /** Secondary text shown beside the label, such as an address. */
  detail?: string
  /** Further text a search finds the option by, beyond its label and detail. */
  keywords?: ReadonlyArray<string | undefined>
}

export interface SearchableOptionGroup<O extends SearchableOption = SearchableOption> {
  /** Heading of the group; `null` for options listed above every group. */
  label: string | null
  options: O[]
}

/**
 * The options whose label, detail or keywords hold every word of `query`,
 * ignoring case; groups left empty are dropped.
 */
export function filterOptionGroups<O extends SearchableOption>(
  groups: ReadonlyArray<SearchableOptionGroup<O>>,
  query: string
): Array<SearchableOptionGroup<O>> {
  return groups
    .map((group) => ({
      ...group,
      options: group.options.filter((option) =>
        matchesQuery(query, [option.label, option.detail, ...(option.keywords ?? [])])
      ),
    }))
    .filter((group) => group.options.length > 0)
}
