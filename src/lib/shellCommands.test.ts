import { describe, expect, it } from "vitest"

import { splitShellCommands } from "@/lib/shellCommands"

const split = (...lines: string[]) => splitShellCommands(lines.join("\n"))

describe("splitShellCommands", () => {
  it("keeps each plain line apart and drops blank ones", () => {
    expect(split("cd /tmp", "", "  ", "ls -la # list", "")).toEqual(["cd /tmp", "ls -la # list"])
  })

  it("keeps a line ending in a backslash with the next", () => {
    expect(split("docker run \\", "  -it alpine", "echo done")).toEqual([
      "docker run \\\n  -it alpine",
      "echo done",
    ])
    // An escaped backslash ends the line.
    expect(split("echo a\\\\", "pwd")).toEqual(["echo a\\\\", "pwd"])
  })

  it("keeps an open quote with the lines up to its end", () => {
    expect(split("echo 'one", "", "two' \"x", 'y"', "pwd")).toEqual([
      "echo 'one\n\ntwo' \"x\ny\"",
      "pwd",
    ])
    expect(split('echo "it\'s"', "pwd")).toEqual(['echo "it\'s"', "pwd"])
  })

  it("keeps a here-document with its body, up to the line that ends it", () => {
    expect(split("cat <<'EOF' > a.txt", "if then", "", "EOF", "pwd")).toEqual([
      "cat <<'EOF' > a.txt\nif then\n\nEOF",
      "pwd",
    ])
    expect(split("cat <<-END", "\tx", "\tEND", "pwd")).toEqual(["cat <<-END\n\tx\n\tEND", "pwd"])
    expect(split("grep x <<< 'y'", "pwd")).toEqual(["grep x <<< 'y'", "pwd"])
  })

  it("keeps a block together, however its lines are laid out", () => {
    expect(split("for f in *.log", "do", "  gzip $f", "done", "ls")).toEqual([
      "for f in *.log\ndo\n  gzip $f\ndone",
      "ls",
    ])
    expect(
      split("if [ -f a ]; then", "  echo a", "elif [ -f b ]; then", "  echo b", "fi", "ls")
    ).toEqual(["if [ -f a ]; then\n  echo a\nelif [ -f b ]; then\n  echo b\nfi", "ls"])
    expect(split("while read l; do echo $l; done < f", "ls")).toEqual([
      "while read l; do echo $l; done < f",
      "ls",
    ])
    expect(split('case "$1" in', "  a) echo a ;;", "  *) echo other ;;", "esac", "ls")).toEqual([
      'case "$1" in\n  a) echo a ;;\n  *) echo other ;;\nesac',
      "ls",
    ])
  })

  it("keeps braces and parentheses that span lines together", () => {
    expect(split("greet() {", '  echo "hi ${1}"', "}", "greet")).toEqual([
      'greet() {\n  echo "hi ${1}"\n}',
      "greet",
    ])
    expect(split("(cd /tmp &&", "  ls)", "pwd")).toEqual(["(cd /tmp &&\n  ls)", "pwd"])
    // PowerShell and fish blocks.
    expect(split("if ($x) {", "  'yes'", "}", "Get-Date")).toEqual([
      "if ($x) {\n  'yes'\n}",
      "Get-Date",
    ])
    expect(split("for f in *.txt", "  echo $f", "end", "ls")).toEqual([
      "for f in *.txt\n  echo $f\nend",
      "ls",
    ])
  })

  it("takes no keyword from an argument, a quote or a comment", () => {
    expect(split("echo if for done", "grep 'while' f", "ls # if", "pwd")).toEqual([
      "echo if for done",
      "grep 'while' f",
      "ls # if",
      "pwd",
    ])
  })

  it("still sends a command left open at the end", () => {
    expect(split("ls", "echo 'oops")).toEqual(["ls", "echo 'oops"])
  })
})
