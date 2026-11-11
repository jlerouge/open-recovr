#!/usr/bin/env bash
# Regenerates the screenshots in docs/screenshots/ with the program's own
# --screenshot mode. The results come from a real quick search: put a few files
# in the Recycle Bin (or the desktop trash) first so there is something to show.
#
#   cargo build --release && scripts/screenshots.sh [disk] [theme]
set -euo pipefail
cd "$(dirname "$0")/.."
exe=target/release/open-recovr
[ -x "$exe.exe" ] && exe="$exe.exe"
disk="${1:-C}"
theme="${2:-clair}" # sombre, nord, catppuccin-mocha…
out=docs/screenshots
mkdir -p "$out"

shot() { # name lang args...
  local name=$1 lang=$2; shift 2
  "$exe" --lang "$lang" --disk "$disk" --theme "$theme" --screenshot "$out/$name.png" "$@"
  echo "$out/$name.png"
}

for lang in fr en es de; do
  shot "$lang-1-start"     "$lang"
  shot "$lang-2-home"      "$lang" --scene home
  shot "$lang-3-scanning"  "$lang" --search --scene scanning
  shot "$lang-4-results"   "$lang" --search
  shot "$lang-5-selected"  "$lang" --search --scene selected
  shot "$lang-6-recovered" "$lang" --search --scene recovered
  shot "$lang-7-expert"    "$lang" --search --expert
  shot "$lang-8-settings"  "$lang" --scene settings
  shot "$lang-9-dark"      "$lang" --search --scene selected --theme sombre
done
