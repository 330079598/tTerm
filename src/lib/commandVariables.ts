import type { SavedCommand, SavedCommandVariable } from "@/types/command"

/** `{{name}}`: a letter or underscore, then letters, digits and underscores. */
const PLACEHOLDER_PATTERN = /\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g

/** What a placeholder looks like, for hints; kept out of the translations because i18next interpolates `{{…}}`. */
export const PLACEHOLDER_EXAMPLE = "{{host}}"

export function formatPlaceholder(name: string): string {
  return `{{${name}}}`
}

/** Placeholder names in `commandText`, each once, in order of first appearance. */
export function extractPlaceholderNames(commandText: string): string[] {
  const names: string[] = []
  for (const match of commandText.matchAll(PLACEHOLDER_PATTERN)) {
    if (!names.includes(match[1])) names.push(match[1])
  }
  return names
}

/** The options of a `choice` variable; empty when none are stored or the value is malformed. */
export function parseChoiceOptions(optionsJson?: string | null): string[] {
  if (!optionsJson) return []
  try {
    const parsed: unknown = JSON.parse(optionsJson)
    return Array.isArray(parsed)
      ? parsed.filter((option): option is string => typeof option === "string")
      : []
  } catch {
    return []
  }
}

/** Splits the editor's one-per-line (or comma separated) options field. */
export function parseChoiceOptionsText(value: string): string[] {
  const options: string[] = []
  for (const option of value.split(/[\n,，]/).map((item) => item.trim())) {
    if (option && !options.includes(option)) options.push(option)
  }
  return options
}

/**
 * The variables to ask for when inserting: those defined on the command whose
 * placeholder still appears in its text. A `{{…}}` without a definition is
 * literal text (Go templates, Jinja), so it is left alone.
 */
export function activeVariables(
  command: Pick<SavedCommand, "commandText" | "variables">
): SavedCommandVariable[] {
  const names = extractPlaceholderNames(command.commandText)
  return command.variables
    .filter((variable) => names.includes(variable.name))
    .sort((left, right) => left.position - right.position)
}

export function initialVariableValues(variables: SavedCommandVariable[]): Record<string, string> {
  return Object.fromEntries(
    variables.map((variable) => {
      if (variable.valueType === "secret") return [variable.name, ""]
      if (variable.valueType === "choice") {
        const options = parseChoiceOptions(variable.optionsJson)
        const fallback = variable.isRequired ? (options[0] ?? "") : ""
        return [
          variable.name,
          variable.defaultValue && options.includes(variable.defaultValue)
            ? variable.defaultValue
            : fallback,
        ]
      }
      return [variable.name, variable.defaultValue ?? ""]
    })
  )
}

export type VariableValueError = "required" | "number"

/** Problems with the values entered for `variables`, keyed by variable name. */
export function validateVariableValues(
  variables: SavedCommandVariable[],
  values: Record<string, string>
): Record<string, VariableValueError> {
  const errors: Record<string, VariableValueError> = {}
  for (const variable of variables) {
    const value = values[variable.name] ?? ""
    if (value.trim() === "") {
      if (variable.isRequired) errors[variable.name] = "required"
    } else if (variable.valueType === "number" && !Number.isFinite(Number(value))) {
      errors[variable.name] = "number"
    }
  }
  return errors
}

/**
 * Replaces the placeholders of `variables` with `values`. With `maskSecrets`
 * a filled-in secret shows as bullets, for previews.
 */
export function renderCommandText(
  commandText: string,
  variables: SavedCommandVariable[],
  values: Record<string, string>,
  options: { maskSecrets?: boolean } = {}
): string {
  const byName = new Map(variables.map((variable) => [variable.name, variable]))
  return commandText.replace(PLACEHOLDER_PATTERN, (placeholder, name: string) => {
    const variable = byName.get(name)
    if (!variable) return placeholder
    const value = values[name] ?? ""
    return options.maskSecrets && variable.valueType === "secret" && value
      ? "•".repeat(Math.min(value.length, 8))
      : value
  })
}
