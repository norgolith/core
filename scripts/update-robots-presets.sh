#!/usr/bin/env bash
set -euo pipefail

ROBOTS_URL="https://raw.githubusercontent.com/ai-robots-txt/ai.robots.txt/main/robots.txt"
TODAY=$(date +%Y-%m-%d)

cd "$(git rev-parse --show-toplevel)"
SEO_FILE="core/src/cmd/seo.rs"

USER_AGENTS=$(curl -sL "$ROBOTS_URL" | grep -i '^User-agent:' | sort -u)

if [ -z "$USER_AGENTS" ]; then
    echo "ERROR: Failed to fetch User-agent lines from $ROBOTS_URL" >&2
    exit 1
fi

BLOCK="// Updated: $TODAY by scripts/update-robots-presets.sh
const ROBOTS_NO_LLMS: &str = r\"${USER_AGENTS}
Disallow: /\";
// END ROBOTS_NO_LLMS_GENERATED"

awk -v block="$BLOCK" '
/^\/\/ BEGIN ROBOTS_NO_LLMS_GENERATED$/ { print; printf "%s", block; skip=1; next }
/^\/\/ END ROBOTS_NO_LLMS_GENERATED$/   { skip=0; next }
!skip
' "$SEO_FILE" > "${SEO_FILE}.tmp" && mv "${SEO_FILE}.tmp" "$SEO_FILE"

echo "Updated $SEO_FILE ($TODAY)"
