#pragma once
#include <QtCore/QString>
#include <QtCore/QStringList>

// Keep in sync with src/media_path.rs. Do not hide arbitrary dotfiles/media.
inline bool kogIsMetadataPath(QString path)
{
    path.replace('\\', '/');
    for (const auto &part : path.split('/')) {
        const auto name = part.toLower();
        if (name.startsWith("._") || name == ".ds_store" || name == "__macosx"
            || name == ".appledouble" || name == ".lsoverride"
            || name == ".spotlight-v100" || name == ".trashes"
            || name == ".fseventsd" || name == ".temporaryitems"
            || name == "thumbs.db" || name == "ehthumbs.db"
            || name == "desktop.ini" || name == "$recycle.bin"
            || name == "system volume information") return true;
    }
    return false;
}
