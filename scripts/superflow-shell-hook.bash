# SuperFlow context anchor (bash)
#
# Publishes the shell's working directory on every prompt so SuperFlow can
# resolve spoken file references against the exact project in front of you.
# Writes ONLY $PWD; no history, no output, nothing else.
#
# Install: source this file from ~/.bashrc.

_superflow_publish_pwd() {
  local dir="${TMPDIR:-/tmp}/superflow"
  mkdir -p "$dir" 2>/dev/null || return 0
  [ -w "$dir" ] || return 0
  printf '%s' "$PWD" > "$dir/cwd.$$" 2>/dev/null \
    && mv -f "$dir/cwd.$$" "$dir/cwd" 2>/dev/null
}

case "${PROMPT_COMMAND:-}" in
  *_superflow_publish_pwd*) ;;
  "") PROMPT_COMMAND="_superflow_publish_pwd" ;;
  *) PROMPT_COMMAND="_superflow_publish_pwd; ${PROMPT_COMMAND}" ;;
esac
