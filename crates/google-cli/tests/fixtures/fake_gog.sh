#!/bin/sh
set -eu

if [ -n "${FAKE_GOG_LOG:-}" ]; then
  : >"$FAKE_GOG_LOG"
  for arg in "$@"; do
    printf '%s\n' "$arg" >>"$FAKE_GOG_LOG"
  done
fi

if [ -n "${FAKE_GOG_STDERR:-}" ]; then
  printf '%s' "$FAKE_GOG_STDERR" >&2
fi

if [ -n "${FAKE_GOG_STDOUT:-}" ]; then
  printf '%s' "$FAKE_GOG_STDOUT"
fi

exit "${FAKE_GOG_EXIT_CODE:-0}"
