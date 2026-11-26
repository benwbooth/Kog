#include "kog_single_instance.h"

#include <QtCore/QCryptographicHash>
#include <QtCore/QDir>
#include <QtCore/QLockFile>
#include <QtCore/QMetaObject>
#include <QtCore/QObject>
#include <QtCore/QVariant>
#include <QtCore/QStandardPaths>
#include <QtCore/QSysInfo>
#include <QtCore/QThread>
#include <QtCore/QTimer>
#include <QtGui/QGuiApplication>
#include <QtGui/QWindow>
#include <QtNetwork/QLocalServer>
#include <QtNetwork/QLocalSocket>

#include <memory>

#ifdef Q_OS_UNIX
#include <unistd.h>
#endif

namespace {

// Keep the lock alive for the entire lifetime of the GUI. QLockFile removes
// it on clean exit and detects a dead owner's PID after a crash.
std::unique_ptr<QLockFile> guiLock;

QString serverName()
{
#ifdef Q_OS_UNIX
    const QByteArray user = QByteArray::number(geteuid());
#else
    const QByteArray user = QDir::homePath().toUtf8();
#endif
    const QByteArray identity = user + '|' + QSysInfo::machineHostName().toUtf8();
    const QByteArray digest = QCryptographicHash::hash(identity, QCryptographicHash::Sha256)
                                  .toHex().left(16);
    return QStringLiteral("kog-gui-") + QString::fromLatin1(digest);
}

QString serverAddress(const QString &name)
{
#ifdef Q_OS_UNIX
    // QLocalServer otherwise places a short name under TMPDIR. Nix shells
    // use private TMPDIRs, so separately launched Kog processes miss each
    // other even when they belong to the same user.
    return QStringLiteral("/tmp/") + name + QStringLiteral(".sock");
#else
    return name;
#endif
}

QWindow *mainWindow()
{
    QWindow *visiblePlayer = nullptr;
    QWindow *mainPlayer = nullptr;
    for (QWindow *window : QGuiApplication::topLevelWindows()) {
        const QString objectName = window->objectName();
        const bool playerWindow = objectName == QStringLiteral("kogMainWindow")
            || objectName == QStringLiteral("kogMiniPlayer")
            || objectName == QStringLiteral("kogClassicPlayer")
            || objectName == QStringLiteral("kogModernPlayer");
        if (!playerWindow) {
            continue;
        }
        if (window->isActive()) {
            return window;
        }
        if (window->isVisible() && !visiblePlayer) {
            visiblePlayer = window;
        }
        if (objectName == QStringLiteral("kogMainWindow")) {
            mainPlayer = window;
        }
    }
    return visiblePlayer ? visiblePlayer : mainPlayer;
}

bool raiseMainWindow()
{
    QWindow *window = mainWindow();
    if (!window) {
        return false;
    }
    if (window->objectName() == QStringLiteral("kogMainWindow")
        && (window->visibility() == QWindow::Minimized || !window->isVisible())) {
        // Main.qml already knows how to restore a maximized window and leave
        // tray mode. Reuse that path when a second launch wakes the app.
        if (!QMetaObject::invokeMethod(window, "showFromTray")) {
            window->showNormal();
        }
    } else if (window->visibility() == QWindow::Minimized) {
        window->showNormal();
    } else if (!window->isVisible()) {
        window->show();
    }
    window->raise();
    window->requestActivate();
    QTimer::singleShot(250, window, [window] {
        if (!window->isActive()) {
            window->alert(0);
        }
    });
    return true;
}

QWindow *playerRoot()
{
    for (QWindow *window : QGuiApplication::topLevelWindows()) {
        if (window->objectName() == QStringLiteral("kogMainWindow")) {
            return window;
        }
    }
    return nullptr;
}

// Requests are read only once the event loop runs, after Main.qml loaded, so
// the window is there unless loading it failed.
void deliverFiles(const QString &filesJson)
{
    if (filesJson.isEmpty()) {
        return;
    }
    QWindow *window = playerRoot();
    if (!window || !QMetaObject::invokeMethod(window, "openFiles",
                                              Q_ARG(QVariant, QVariant(filesJson)))) {
        qWarning("Kog: no main window to open files in");
    }
}

enum class RaiseResult { Unavailable, Raised, Rejected };

// The request is "R" (raise) or "RF" + a JSON array of paths + "\n" (raise and
// play them). Kogs older than the files request read only the "R".
RaiseResult requestRaise(const QString &name, const QString &filesJson)
{
    QLocalSocket socket;
    socket.connectToServer(name);
    if (!socket.waitForConnected(100)) {
        return RaiseResult::Unavailable;
    }
    QByteArray request("R");
    if (!filesJson.isEmpty()) {
        request += 'F' + filesJson.toUtf8() + '\n';
    }
    if (socket.write(request) != request.size()) {
        return RaiseResult::Rejected;
    }
    while (socket.bytesToWrite() > 0) {
        if (!socket.waitForBytesWritten(1000)) {
            return RaiseResult::Rejected;
        }
    }
    if (!socket.waitForReadyRead(5000)) {
        return RaiseResult::Rejected;
    }
    return socket.readAll().startsWith("OK") ? RaiseResult::Raised : RaiseResult::Rejected;
}

void acceptRaiseRequests(QLocalServer *server)
{
    while (QLocalSocket *socket = server->nextPendingConnection()) {
        auto buffer = std::make_shared<QByteArray>();
        auto handleRequest = [socket, buffer] {
            if (socket->bytesAvailable() == 0) {
                return;
            }
            buffer->append(socket->readAll());
            // A files request is complete at its newline.
            if (buffer->startsWith("RF") && !buffer->contains('\n')) {
                return;
            }
            const bool valid = buffer->contains('R');
            if (buffer->startsWith("RF")) {
                const QByteArray json = buffer->mid(2, buffer->indexOf('\n') - 2);
                // After replying: playing them may take a moment, and the
                // launching process waits only so long.
                QTimer::singleShot(0, qApp, [json] { deliverFiles(QString::fromUtf8(json)); });
            }
            buffer->clear();
            socket->write(valid && raiseMainWindow() ? "OK\n" : "NO_WINDOW\n");
            socket->flush();
            socket->disconnectFromServer();
        };
        QObject::connect(socket, &QLocalSocket::readyRead, socket, handleRequest);
        QObject::connect(socket, &QLocalSocket::disconnected, socket, &QObject::deleteLater);
        handleRequest();
    }
}

} // namespace

void kogOpenFiles(const QString &filesJson)
{
    deliverFiles(filesJson);
}

QString kogSingleInstanceStart(const QString &filesJson)
{
    if (guiLock) {
        return QStringLiteral("primary");
    }
#ifdef Q_OS_UNIX
    // The lock must not follow XDG_DATA_HOME: the same account can launch
    // Kog from different shells with different environment settings.
    const QString directory = QStringLiteral("/tmp");
#else
    const QString directory = QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation);
#endif
    if (directory.isEmpty() || !QDir().mkpath(directory)) {
        return QStringLiteral("cannot create Kog's per-user runtime directory");
    }
    const QString name = serverName();
    const QString address = serverAddress(name);
    auto lock = std::make_unique<QLockFile>(QDir(directory).filePath(name + QStringLiteral(".lock")));
    lock->setStaleLockTime(0);

    bool acquired = false;
    for (int attempt = 0; attempt < 40; ++attempt) {
        if (lock->tryLock(0)) {
            acquired = true;
            break;
        }
        if (lock->error() != QLockFile::LockFailedError) {
            return QStringLiteral("cannot lock Kog's per-user GUI instance: ") + lock->fileName();
        }
        switch (requestRaise(address, filesJson)) {
        case RaiseResult::Raised:
            return QStringLiteral("raised");
        case RaiseResult::Rejected:
            return QStringLiteral("the existing Kog window did not accept the raise request");
        case RaiseResult::Unavailable:
            QThread::msleep(50);
            break;
        }
    }
    if (!acquired) {
        return QStringLiteral("the existing Kog GUI is locked but did not respond");
    }

    auto *server = new QLocalServer(qApp);
    server->setSocketOptions(QLocalServer::UserAccessOption);
    if (!server->listen(address)) {
        // A socket can remain after a killed GUI. Check for an answering
        // server before removing it, even though we now hold the user lock.
        switch (requestRaise(address, filesJson)) {
        case RaiseResult::Raised:
            delete server;
            return QStringLiteral("raised");
        case RaiseResult::Rejected:
            delete server;
            return QStringLiteral("the existing Kog window did not accept the raise request");
        case RaiseResult::Unavailable:
            break;
        }
        // We hold the lock, so an unreachable socket is left by a dead GUI.
        QLocalServer::removeServer(address);
        if (!server->listen(address)) {
            const QString error = server->errorString();
            delete server;
            return QStringLiteral("cannot listen for Kog GUI activation: ") + error;
        }
    }
    QObject::connect(server, &QLocalServer::newConnection, server,
                     [server] { acceptRaiseRequests(server); });
    guiLock = std::move(lock);
    return QStringLiteral("primary");
}
