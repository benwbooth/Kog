#include "kog_desktop_integration.h"
#include "kog_modern_skin.h"

#include <QtCore/QFileInfo>
#include <QtCore/QMimeDatabase>
#include <QtCore/QSettings>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QWindow>

#include <array>

std::unique_ptr<QApplication> kogApplicationNew()
{
    kogInitializeModernSkins();
    static QByteArray executableName("kog");
    static QByteArray sessionOption("-session");
    static QByteArray sessionId;
    static std::array<char *, 4> arguments { executableName.data(), nullptr, nullptr, nullptr };
    static int argumentCount = 1;
#ifdef KOG_WAYLAND_SESSION_RESTORE
    if (qEnvironmentVariable("XDG_SESSION_TYPE") == QStringLiteral("wayland")
        && !qEnvironmentVariable("QT_QPA_PLATFORM").startsWith(QStringLiteral("xcb"))) {
        sessionId = QSettings("Kog", "Kog").value("MainWindow/waylandSessionId").toByteArray();
        if (!sessionId.isEmpty()) {
            arguments[1] = sessionOption.data();
            arguments[2] = sessionId.data();
            argumentCount = 3;
        }
    }
#endif
    auto application = std::make_unique<QApplication>(argumentCount, arguments.data());
    kogRegisterModernSkinTypes();
    application->setOrganizationName(QStringLiteral("Kog"));
#ifdef KOG_WAYLAND_SESSION_RESTORE
    if (application->platformName().startsWith(QStringLiteral("wayland"))
        && !application->sessionId().isEmpty()) {
        QSettings settings("Kog", "Kog");
        settings.setValue("MainWindow/waylandSessionId", application->sessionId());
        settings.sync();
    }
#endif
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
