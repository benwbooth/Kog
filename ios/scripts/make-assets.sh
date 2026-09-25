#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
assets="$repo/ios/Kog/Assets.xcassets"
mkdir -p "$assets/AppIcon.appiconset"
cat > "$assets/AppIcon.appiconset/Contents.json" <<'EOF'
{"images":[{"filename":"Kog-1024.png","idiom":"universal","platform":"ios","size":"1024x1024"}],"info":{"author":"xcode","version":1}}
EOF
magick -background '#202427' -density 512 "$repo/crates/kog-web/icons/kog.svg" \
  -resize 1024x1024 -alpha remove -alpha off "$assets/AppIcon.appiconset/Kog-1024.png"
for source in "$repo"/android/app/src/main/res/drawable-nodpi/kog_*.png; do
  name=$(basename "$source" .png)
  directory="$assets/$name.imageset"
  mkdir -p "$directory"
  cp "$source" "$directory/$name.png"
  printf '{"images":[{"filename":"%s.png","idiom":"universal"}],"info":{"author":"xcode","version":1}}\n' "$name" > "$directory/Contents.json"
done
