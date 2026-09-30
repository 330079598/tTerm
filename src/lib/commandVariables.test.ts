import { describe, expect, it } from "vitest"

import {
  activeVariables,
  extractPlaceholderNames,
  initialVariableValues,
  parseChoiceOptions,
  parseChoiceOptionsText,
  renderCommandText,
  validateVariableValues,
} from "@/lib/commandVariables"
import type { SavedCommandVariable } from "@/types/command"

function variable(
  name: string,
  overrides: Partial<SavedCommandVariable> = {}
): SavedCommandVariable {
  return { name, label: name, valueType: "text", isRequired: true, position: 0, ...overrides }
}

describe("extractPlaceholderNames", () => {
  it("lists each placeholder once, in order of first appearance", () => {
    expect(extractPlaceholderNames("ssh {{user}}@{{ host }} -p {{port}} # {{user}}")).toEqual([
      "user",
      "host",
      "port",
    ])
  })

  it("ignores template syntax that is not a plain name", () => {
    expect(
      extractPlaceholderNames("docker inspect -f '{{.State.Status}} {{range .Mounts}}' {{1st}} {}")
    ).toEqual([])
  })
})

describe("choice options", () => {
  it("parses stored options and tolerates malformed values", () => {
    expect(parseChoiceOptions('["dev","prod"]')).toEqual(["dev", "prod"])
    expect(parseChoiceOptions('{"a":1}')).toEqual([])
    expect(parseChoiceOptions("not json")).toEqual([])
    expect(parseChoiceOptions(undefined)).toEqual([])
  })

  it("splits the editor field on lines and commas without duplicates", () => {
    expect(parseChoiceOptionsText("dev, prod\nstaging，dev\n\n")).toEqual([
      "dev",
      "prod",
      "staging",
    ])
  })
})

describe("activeVariables", () => {
  it("keeps defined variables still present in the text, by position", () => {
    const command = {
      commandText: "kubectl -n {{namespace}} logs {{pod}} '{{end}}'",
      variables: [
        variable("pod", { position: 1 }),
        variable("removed", { position: 2 }),
        variable("namespace", { position: 0 }),
      ],
    }
    expect(activeVariables(command).map((item) => item.name)).toEqual(["namespace", "pod"])
  })
})

describe("initialVariableValues", () => {
  it("prefills defaults, never secrets, and picks a valid choice", () => {
    expect(
      initialVariableValues([
        variable("host", { defaultValue: "example.com" }),
        variable("token", { valueType: "secret" }),
        variable("env", { valueType: "choice", optionsJson: '["dev","prod"]', defaultValue: "qa" }),
        variable("tier", {
          valueType: "choice",
          optionsJson: '["a","b"]',
          defaultValue: "b",
        }),
        variable("zone", { valueType: "choice", optionsJson: '["x"]', isRequired: false }),
      ])
    ).toEqual({ host: "example.com", token: "", env: "dev", tier: "b", zone: "" })
  })
})

describe("validateVariableValues", () => {
  it("reports missing required values and non-numeric numbers", () => {
    const variables = [
      variable("host"),
      variable("note", { isRequired: false }),
      variable("port", { valueType: "number" }),
      variable("count", { valueType: "number", isRequired: false }),
    ]
    expect(validateVariableValues(variables, { host: " ", port: "22x", count: "" })).toEqual({
      host: "required",
      port: "number",
    })
    expect(validateVariableValues(variables, { host: "a", port: "2222" })).toEqual({})
  })
})

describe("renderCommandText", () => {
  const variables = [variable("host"), variable("token", { valueType: "secret" })]

  it("replaces defined placeholders and leaves the rest untouched", () => {
    expect(
      renderCommandText("curl -H 'X: {{token}}' {{ host }}/{{host}} '{{end}}'", variables, {
        host: "a.io",
        token: "s3cret",
      })
    ).toBe("curl -H 'X: s3cret' a.io/a.io '{{end}}'")
  })

  it("masks secrets for previews", () => {
    expect(
      renderCommandText(
        "login {{token}} {{host}}",
        variables,
        { host: "a.io", token: "s3cret" },
        {
          maskSecrets: true,
        }
      )
    ).toBe("login •••••• a.io")
  })

  it("does not re-expand placeholders that appear in a value", () => {
    expect(renderCommandText("echo {{host}}", variables, { host: "{{token}}", token: "x" })).toBe(
      "echo {{token}}"
    )
  })
})
