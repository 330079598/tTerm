() { local status=$?; builtin printf '\033]7;file://%s\033\\' "${PWD//\%/%25}"; return $status
}
