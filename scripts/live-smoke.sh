#!/bin/sh
set -eu

if [ "${APPLE_MAIL_LIVE_SMOKE:-}" != "1" ]; then
    echo "Refusing to access Apple Mail. Set APPLE_MAIL_LIVE_SMOKE=1 to run the read-only account-discovery smoke." >&2
    exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required to validate the smoke-test response." >&2
    exit 2
fi

accounts="$(cargo run --quiet -- accounts)"
printf '%s\n' "$accounts" | jq -e '
    type == "array" and
    all(.[];
        (.id | type == "string") and
        (.name | type == "string") and
        (.email_addresses | type == "array") and
        (.enabled | type == "boolean")
    )
' >/dev/null

echo "Read-only Apple Mail account-discovery smoke passed."
