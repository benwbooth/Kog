#pragma once

#include <QtCore/QObject>
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
QString kogDesktopCursorPos();
std::unique_ptr<QApplication> kogApplicationNew();
void kogApplicationSetName(QApplication &application, const QString &name);
void kogApplicationSetVersion(QApplication &application, const QString &version);
int kogApplicationExec(QApplication &application);
void kogApplyApplicationIcon();
void kogRestoreMainWindow();

// The appearance choice in Preferences: "system" follows the desktop between
// Earshot's light and dark palettes, "light" and "dark" keep one of them.
// Registered for QML as the org.kog.native KogAppearance singleton.
class KogAppearance : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString mode READ mode WRITE setMode NOTIFY modeChanged)
public:
    explicit KogAppearance(QObject *parent = nullptr);
    QString mode() const;
    void setMode(const QString &mode);
signals:
    void modeChanged();
};
