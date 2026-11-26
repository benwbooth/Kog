#pragma once

#include <QtCore/QString>

// Returns "primary", "raised", or a diagnostic explaining why no GUI can start.
// filesJson (a JSON array of paths, or empty) goes to the running Kog when one
// is raised.
QString kogSingleInstanceStart(const QString &filesJson);

// Plays the files in filesJson (a JSON array of paths) in this process's main
// window: Main.qml's openFiles().
void kogOpenFiles(const QString &filesJson);
