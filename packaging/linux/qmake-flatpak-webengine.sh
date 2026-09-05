#!/usr/bin/env bash
# cxx-qt obtains its include and linker locations exclusively through
# `qmake -query`. The KDE SDK's qmake describes /usr, while the WebEngine
# BaseApp intentionally adds its development files in /app.
set -euo pipefail

if [[ $# -eq 2 && $1 == "-query" ]]; then
  case $2 in
    QT_INSTALL_PREFIX) echo /app ;;
    QT_INSTALL_HEADERS) echo /app/include ;;
    QT_INSTALL_LIBS) echo /app/lib ;;
    QT_INSTALL_PLUGINS) echo /app/lib/plugins ;;
    *) exec qmake6 "$@" ;;
  esac
  exit 0
fi

exec qmake6 "$@"
