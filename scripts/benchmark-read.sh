#!/bin/sh
set -eu

if [ "${APPLE_MAIL_READ_BENCHMARK:-}" != "1" ]; then
    echo "Refusing to inspect Apple Mail. Set APPLE_MAIL_READ_BENCHMARK=1 to run the read-only benchmark." >&2
    exit 2
fi

for command in cargo jq perl; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "$command is required to run the read-only benchmark." >&2
        exit 2
    fi
done

now_ms() {
    perl -MTime::HiRes=time -e 'printf "%.0f\n", time * 1000'
}

started="$(now_ms)"
doctor="$(cargo run --quiet -- doctor)"
doctor_elapsed_ms=$(( $(now_ms) - started ))

printf '%s\n' "$doctor" | jq -e '
    (.backend_ready == true) and
    (.account_count | type == "number") and
    (.elapsed_ms | type == "number")
' >/dev/null

started="$(now_ms)"
snapshot="$(cargo run --quiet -- inbox --recent-limit 1 --unread-limit 1)"
snapshot_elapsed_ms=$(( $(now_ms) - started ))

started="$(now_ms)"
search="$(cargo run --quiet -- search --mailbox INBOX --limit 1)"
search_elapsed_ms=$(( $(now_ms) - started ))

printf '%s\n' "$snapshot" | jq -e '
    (.total_count | type == "number") and
    (.unread_count | type == "number") and
    (.completeness | type == "string")
' >/dev/null
printf '%s\n' "$search" | jq -e '
    (.scanned_count | type == "number") and
    (.matched_count | type == "number") and
    (.completeness | type == "string")
' >/dev/null

jq -n \
    --argjson account_count "$(printf '%s\n' "$doctor" | jq '.account_count')" \
    --argjson doctor_elapsed_ms "$doctor_elapsed_ms" \
    --argjson total_count "$(printf '%s\n' "$snapshot" | jq '.total_count')" \
    --argjson unread_count "$(printf '%s\n' "$snapshot" | jq '.unread_count')" \
    --arg snapshot_completeness "$(printf '%s\n' "$snapshot" | jq -r '.completeness')" \
    --argjson snapshot_elapsed_ms "$snapshot_elapsed_ms" \
    --argjson scanned_count "$(printf '%s\n' "$search" | jq '.scanned_count')" \
    --argjson matched_count "$(printf '%s\n' "$search" | jq '.matched_count')" \
    --arg search_completeness "$(printf '%s\n' "$search" | jq -r '.completeness')" \
    --argjson search_elapsed_ms "$search_elapsed_ms" \
    '{
        account_count: $account_count,
        doctor: {
            elapsed_ms: $doctor_elapsed_ms
        },
        inbox_snapshot: {
            total_count: $total_count,
            unread_count: $unread_count,
            completeness: $snapshot_completeness,
            elapsed_ms: $snapshot_elapsed_ms
        },
        search: {
            scanned_count: $scanned_count,
            matched_count: $matched_count,
            completeness: $search_completeness,
            elapsed_ms: $search_elapsed_ms
        }
    }'
