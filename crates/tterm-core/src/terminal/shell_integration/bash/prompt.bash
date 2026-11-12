() { local status=$?; builtin printf '\033]133;D;%s\033\\\033]7;file://%s\033\\\033]133;A\033\\' "$status" "${PWD//\%/%25}"; return $status
}
