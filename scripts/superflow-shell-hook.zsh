# SuperFlow context anchor (zsh)
#
# Publishes the shell's working directory on every prompt so SuperFlow can
# resolve spoken file references ("fix the payment file") against the exact
# project in front of you — including which tab/pane you're in. It writes
# ONLY $PWD; no history, no output, nothing else.
#
# Install:
#   source /Applications/SuperFlow.app/Contents/Resources/superflow-shell-hook.zsh
# or copy this file anywhere and add that line to ~/.zshrc.

_superflow_publish_pwd() {
  local dir="${TMPDIR:-/tmp}/superflow"
  mkdir -p "$dir" 2>/dev/null || return 0
  [ -w "$dir" ] || return 0
  # Atomic rename: readers never observe a partially written path.
  printf '%s' "$PWD" > "$dir/cwd.$$" 2>/dev/null \
    && mv -f "$dir/cwd.$$" "$dir/cwd" 2>/dev/null
}

autoload -Uz add-zsh-hook 2>/dev/null
add-zsh-hook precmd _superflow_publish_pwd 2>/dev/null
