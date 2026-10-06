#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_dir"

# Run inside nix develop. Include the shell's matching KDE dependencies,
# rather than importing a second Qt/KDE installation from the desktop session.
: "${QML_IMPORT_PATH:?Run this script inside nix develop}"
IFS=: read -ra prefixes <<< "${QT_ADDITIONAL_PACKAGES_PREFIX_PATH:-}"
for prefix in "${prefixes[@]}"; do
    if [[ -d "$prefix/lib/qt-6/qml" ]]; then
        QML_IMPORT_PATH="$QML_IMPORT_PATH:$prefix/lib/qt-6/qml"
    fi
done
export QML_IMPORT_PATH QML2_IMPORT_PATH="$QML_IMPORT_PATH"
unset NIXPKGS_QT6_QML_IMPORT_PATH QT_STYLE_OVERRIDE
export QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=basic QT_QUICK_BACKEND=software
for style in Basic org.kde.desktop; do
    QT_QUICK_CONTROLS_STYLE="$style" qmltestrunner -input tests/qml/tst_PlaylistWorkspace.qml "$@"
done
