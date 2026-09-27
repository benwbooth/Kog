#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 REPOSITORY_ARCHIVE SITE_DIRECTORY PUBLIC_URL" >&2
  exit 2
fi

archive="$(realpath "$1")"
site="$2"
public_url="${3%/}"
script_dir="$(cd "$(dirname "$0")" && pwd)"
key_file="$script_dir/kog-flatpak.gpg.asc"
key_id="$(gpg --show-keys --with-colons "$key_file" | awk -F: '$1 == "fpr" { print $10; exit }')"
test -n "$key_id"
gpg --list-secret-keys "$key_id" >/dev/null
# Refuse to overwrite an existing site or repository.
mkdir "$site"
site="$(realpath "$site")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
tar --zstd -xf "$archive" -C "$work"
mv "$work/repo" "$site/flatpak"
repo="$site/flatpak"
app_ref=app/org.kog.player/x86_64/master
ostree --repo="$repo" rev-parse "$app_ref"

# Only publish the installable application. Debug/source extensions remain in
# the release archive; pruning them keeps the Pages site below its 1 GB limit.
while IFS= read -r ref; do
  if [[ "$ref" != "$app_ref" ]]; then
    ostree --repo="$repo" refs --delete "$ref"
  fi
done < <(ostree --repo="$repo" refs)
flatpak build-sign --arch=x86_64 --gpg-sign="$key_id" "$repo" org.kog.player master
flatpak build-update-repo --title=Kog --default-branch=master \
  --homepage=https://github.com/benwbooth/Kog \
  --icon="$public_url/kog.svg" --gpg-sign="$key_id" \
  --generate-static-deltas --prune --prune-depth=0 "$repo"

gpg --dearmor < "$key_file" > "$site/kog.gpg"
key_base64="$(base64 --wrap=0 "$site/kog.gpg")"
cp "$script_dir/../../qml/icons/kog.svg" "$site/kog.svg"
cp "$script_dir/flatpak-index.html" "$site/index.html"
touch "$site/.nojekyll"
cat > "$site/kog.flatpakrepo" <<EOF
[Flatpak Repo]
Title=Kog
Url=$public_url/flatpak
Homepage=https://github.com/benwbooth/Kog
Comment=Kog music player releases
Description=Albums, audiobooks, MIDI, chiptunes, and game soundtracks in one player.
Icon=$public_url/kog.svg
DefaultBranch=master
GPGKey=$key_base64
EOF
cat > "$site/kog.flatpakref" <<EOF
[Flatpak Ref]
Title=Kog
Name=org.kog.player
Branch=master
Url=$public_url/flatpak
SuggestRemoteName=kog
Homepage=https://github.com/benwbooth/Kog
Icon=$public_url/kog.svg
RuntimeRepo=https://flathub.org/repo/flathub.flatpakrepo
IsRuntime=false
GPGKey=$key_base64
EOF

size="$(du -sb "$site" | cut -f1)"
echo "Flatpak site: $size bytes"
if (( size > 950000000 )); then
  echo 'Flatpak site exceeds the GitHub Pages size budget' >&2
  exit 1
fi

# Verify the generated remote with a fresh keyring/installation, leaving the
# builder's own Flatpak configuration alone. Both commit and summary must verify.
FLATPAK_USER_DIR="$work/client" flatpak remote-add --user \
  --gpg-import="$site/kog.gpg" kog "file://$repo"
FLATPAK_USER_DIR="$work/client" flatpak remote-info --user kog org.kog.player
