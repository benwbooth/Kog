#pragma once

#include <QtCore/QString>
#include <QtWidgets/QApplication>

#include <memory>

QString kogFileIconName(const QString &path);
// Kog's own format icon for a lowercase file suffix without the dot, as
// "kog-format-<key>" resolving to qml/icons/kog-format-<key>[-light].svg, or
// "kog-format-paper" when the suffix is playable but has no dedicated art
// (the row then badges it with the extension), or empty when Kog has no
// opinion and the system theme should answer. The single table keeps the
// tree, the playlist pane, and the Rust icon_name() path in agreement; the
// web client mirrors it in crates/kog-web/src/lib.rs.
QString kogFormatIconName(const QString &suffix);
std::unique_ptr<QApplication> kogApplicationNew();
void kogApplicationSetName(QApplication &application, const QString &name);
void kogApplicationSetVersion(QApplication &application, const QString &version);
int kogApplicationExec(QApplication &application);
void kogApplyApplicationIcon();
void kogRestoreMainWindow();
