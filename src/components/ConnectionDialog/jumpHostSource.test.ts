import { describe, expect, it } from "vitest"

import {
  jumpHostCandidateGroups,
  jumpHostFromProfile,
} from "@/components/ConnectionDialog/jumpHostSource"
import type { SavedProfile } from "@/types/tab"

const profile = (overrides: Partial<SavedProfile>): SavedProfile => ({
  id: "id",
  name: "name",
  connection_type: "ssh",
  host: "host",
  port: 22,
  username: "user",
  remember_password: false,
  keepalive_interval_secs: 30,
  keepalive_count_max: 3,
  ...overrides,
})

describe("jumpHostCandidateGroups", () => {
  it("lists other SSH connections under their groups", () => {
    const groups = jumpHostCandidateGroups(
      [
        profile({ id: "self" }),
        profile({ id: "a", group_path: "prod" }),
        profile({ id: "serial", connection_type: "serial" }),
        profile({ id: "b" }),
        profile({ id: "c", group_path: "prod" }),
        profile({ id: "no-host", host: " " }),
      ],
      "self"
    )
    expect(groups.map(({ group, profiles }) => [group, profiles.map((p) => p.id)])).toEqual([
      ["prod", ["a", "c"]],
      ["", ["b"]],
    ])
  })
})

describe("jumpHostFromProfile", () => {
  it("copies the address and login and drops what was typed", () => {
    expect(
      jumpHostFromProfile(
        profile({
          id: "bastion",
          host: " bastion.example.com ",
          port: 2222,
          username: "ops",
          auth_method: "key",
          private_key_path: "/keys/ops",
        })
      )
    ).toMatchObject({
      profileId: "bastion",
      host: "bastion.example.com",
      port: 2222,
      username: "ops",
      authMethod: "key",
      privateKeyPath: "/keys/ops",
      password: "",
      privateKeyPassphrase: "",
    })
  })
})
