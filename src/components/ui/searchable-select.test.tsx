// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest"

import { SearchableSelect, type SearchableSelectGroup } from "@/components/ui/searchable-select"

const groups: SearchableSelectGroup[] = [
  { label: null, options: [{ value: "", label: "Enter manually" }] },
  {
    label: "prod / web",
    options: [
      { value: "a", label: "bastion", detail: "ops@10.0.0.1:22" },
      { value: "b", label: "web", detail: "deploy@10.0.0.2:22" },
    ],
  },
]

function renderSelect(onChange = vi.fn()) {
  render(
    <SearchableSelect
      id="pick"
      value=""
      groups={groups}
      onChange={onChange}
      triggerLabel="Enter manually"
      searchLabel="Search"
      listLabel="Connect as"
      emptyText="No match."
    />
  )
  fireEvent.click(screen.getByRole("button", { name: "Enter manually" }))
  return { onChange, search: screen.getByRole("textbox", { name: "Search" }) }
}

describe("SearchableSelect", () => {
  beforeAll(() => {
    // jsdom lays nothing out, so it has no scrollIntoView.
    Element.prototype.scrollIntoView = vi.fn()
  })
  afterEach(cleanup)

  it("filters by the detail and picks the highlighted option with Enter", () => {
    const { onChange, search } = renderSelect()
    fireEvent.change(search, { target: { value: "10.0.0.2" } })
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
      "webdeploy@10.0.0.2:22",
    ])
    fireEvent.keyDown(search, { key: "Enter" })
    expect(onChange).toHaveBeenCalledWith("b")
    expect(screen.queryByRole("listbox")).toBeNull()
  })

  it("moves with the arrow keys and points the search at an option by a valid id", () => {
    const { onChange, search } = renderSelect()
    fireEvent.keyDown(search, { key: "ArrowDown" })
    const active = search.getAttribute("aria-activedescendant") ?? ""
    expect(active).not.toMatch(/\s/)
    expect(document.getElementById(active)?.textContent).toBe("bastionops@10.0.0.1:22")
    fireEvent.keyDown(search, { key: "Enter" })
    expect(onChange).toHaveBeenCalledWith("a")
  })

  it("says when nothing matches and closes on Escape without picking", () => {
    const { onChange, search } = renderSelect()
    fireEvent.change(search, { target: { value: "nothing like this" } })
    expect(screen.getByText("No match.")).toBeTruthy()
    fireEvent.keyDown(search, { key: "Escape" })
    expect(screen.queryByRole("listbox")).toBeNull()
    expect(onChange).not.toHaveBeenCalled()
  })
})
