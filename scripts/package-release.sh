#!/bin/sh
set -eu

version="${1:-}"
binary="${2:-}"
output_root="${3:-dist}"

if [ -z "$version" ] || [ -z "$binary" ]; then
    echo "usage: $0 VERSION BINARY [OUTPUT_DIRECTORY]" >&2
    exit 2
fi

case "$version" in
    *[!0-9.]* | .* | *..* | *.)
        echo "VERSION must contain only dot-separated digits." >&2
        exit 2
        ;;
esac

if [ ! -x "$binary" ]; then
    echo "BINARY must be an existing executable." >&2
    exit 2
fi

if ! command -v lipo >/dev/null 2>&1; then
    echo "lipo is required to verify the release executable." >&2
    exit 2
fi

if ! lipo "$binary" -verify_arch arm64 x86_64; then
    echo "BINARY must contain both arm64 and x86_64 architectures." >&2
    exit 2
fi

package_name="apple-mail-mcp-${version}-macos-universal"
stage="${output_root}/${package_name}"
archive="${output_root}/${package_name}.tar.gz"
checksum="${archive}.sha256"
formula="${output_root}/apple-mail-mcp.rb"

for path in "$stage" "$archive" "$checksum" "$formula"; do
    if [ -e "$path" ]; then
        echo "Refusing to overwrite existing release path: $path" >&2
        exit 2
    fi
done

mkdir -p "$stage"
cp "$binary" "$stage/apple-mail"
chmod 755 "$stage/apple-mail"
cp LICENSE README.md "$stage/"
cp packaging/mcp/server.json "$stage/mcp-server.json"
cp packaging/mcp/README.md "$stage/MCP-CONFIG.md"

tar -czf "$archive" -C "$output_root" "$package_name"
archive_sha="$(shasum -a 256 "$archive" | awk '{print $1}')"
printf '%s  %s\n' "$archive_sha" "$(basename "$archive")" > "$checksum"

sed \
    -e "s/@VERSION@/${version}/g" \
    -e "s/@SHA256@/${archive_sha}/g" \
    packaging/homebrew/apple-mail-mcp.rb.in > "$formula"

(
    cd "$output_root"
    shasum -a 256 -c "$(basename "$checksum")"
)
echo "Packaged $archive"
