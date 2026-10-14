#!/usr/bin/env bash
# Builds, signs and publishes a Kura release from this Mac, like Misty's
# releases: nothing builds on hosted CI.
#
#   scripts/release.sh 0.1.0
#
# Needs:
#   TAURI_SIGNING_PRIVATE_KEY (+ _PASSWORD)  updater key; its public half is in
#                                            src-tauri/tauri.conf.json
#   APPLE_SIGNING_IDENTITY                   "Developer ID Application: …"
#   APPLE_API_KEY, APPLE_API_ISSUER,
#   APPLE_API_KEY_PATH                       notarization (App Store Connect API key)
#   gh                                       signed in to github.com/misty-org/kura
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

version="${1:?usage: scripts/release.sh X.Y.Z}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || { echo "Invalid version: $version" >&2; exit 2; }
: "${TAURI_SIGNING_PRIVATE_KEY:?set TAURI_SIGNING_PRIVATE_KEY}"
: "${APPLE_SIGNING_IDENTITY:?set APPLE_SIGNING_IDENTITY}"
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Commit or stash your changes first." >&2
  exit 1
fi
if grep -q '"pubkey": ""' src-tauri/tauri.conf.json; then
  echo "Add the updater public key to src-tauri/tauri.conf.json (npx tauri signer generate)." >&2
  exit 1
fi

# One version everywhere.
node -e '
const fs = require("fs");
const v = process.argv[1];
for (const file of ["package.json", "src-tauri/tauri.conf.json"]) {
  const json = JSON.parse(fs.readFileSync(file, "utf8"));
  json.version = v;
  fs.writeFileSync(file, JSON.stringify(json, null, 2) + "\n");
}
' "$version"
VERSION="$version" perl -0pi -e 's/^version = ".*?"/version = "$ENV{VERSION}"/m' src-tauri/Cargo.toml

.githooks/checks.sh all
npm ci --no-audit --no-fund
npx tauri build --bundles app,dmg --target universal-apple-darwin

bundle="src-tauri/target/universal-apple-darwin/release/bundle"
app_archive="$(ls "$bundle"/macos/*.app.tar.gz)"
dmg="$(ls "$bundle"/dmg/*.dmg)"
signature="$(cat "$app_archive.sig")"
notes="Kura $version"
published="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
url="https://github.com/misty-org/kura/releases/download/kura-v$version/$(basename "$app_archive")"

cat > "$bundle/latest.json" <<JSON
{
  "version": "$version",
  "notes": "$notes",
  "pub_date": "$published",
  "platforms": {
    "darwin-aarch64": { "signature": "$signature", "url": "$url" },
    "darwin-x86_64": { "signature": "$signature", "url": "$url" }
  }
}
JSON

git commit -am "Release Kura $version"
git tag "kura-v$version"
git push --follow-tags
gh release create "kura-v$version" "$dmg" "$app_archive" "$app_archive.sig" "$bundle/latest.json" \
  --repo misty-org/kura --title "Kura $version" --notes "$notes"
