#include "kog_desktop_integration.h"

#include <QtCore/QFileInfo>
#include <QtCore/QMimeDatabase>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QWindow>

#include <array>

std::unique_ptr<QApplication> kogApplicationNew()
{
    static std::array<char, 4> executableName { 'k', 'o', 'g', '\0' };
    static char *arguments[] { executableName.data(), nullptr };
    static int argumentCount = 1;
    auto application = std::make_unique<QApplication>(argumentCount, arguments);
    application->setOrganizationName(QStringLiteral("Kog"));
    return application;
}

void kogApplicationSetName(QApplication &application, const QString &name)
{
    application.setApplicationName(name);
}

int kogApplicationExec(QApplication &application)
{
    return application.exec();
}

QString kogFileIconName(const QString &path)
{
    const QFileInfo fileInfo(path);
    if (fileInfo.isDir()) {
        return QStringLiteral("folder");
    }

    const QMimeDatabase database;
    const auto mimeType = database.mimeTypeForFile(fileInfo, QMimeDatabase::MatchExtension);
    auto iconName = mimeType.iconName();
    if (iconName.isEmpty()) {
        iconName = mimeType.genericIconName();
    }
    if (iconName.isEmpty()) {
        iconName = mimeType.name().startsWith(QStringLiteral("audio/"))
            ? QStringLiteral("audio-x-generic")
            : QStringLiteral("text-x-generic");
    }
    return iconName;
}

void kogApplyApplicationIcon()
{
    const QIcon icon(QStringLiteral(":/qt/qml/org/kog/player/qml/icons/kog.svg"));
    if (icon.isNull()) {
        return;
    }

    QGuiApplication::setWindowIcon(icon);
    for (QWindow *window : QGuiApplication::allWindows()) {
        window->setIcon(icon);
    }
}
