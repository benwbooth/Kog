#include "kog_desktop_integration.h"

#include <QtCore/QSettings>
#include <QtCore/QTemporaryDir>
#include <QtGui/QScreen>
#include <QtGui/QWindow>
#include <QtTest/QTest>
#include <cstdio>
#include <cstdlib>

static void require(bool condition, const char *message)
{
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", message);
        std::exit(1);
    }
}

class MainWindow : public QWindow {
public:
    MainWindow()
    {
        setFlags(Qt::Window | Qt::FramelessWindowHint);
        setObjectName("kogMainWindow");
    }
};

int main(int argc, char **argv)
{
    QApplication application(argc, argv);
    kogApplicationSetVersion(application, QStringLiteral("9.8.7-test"));
    require(QCoreApplication::applicationVersion() == QStringLiteral("9.8.7-test"),
            "About version reaches Qt application metadata");
    QTemporaryDir config;
    require(config.isValid(), "temporary settings directory");
    QSettings::setDefaultFormat(QSettings::IniFormat);
    QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, config.path());
    QSettings::setPath(QSettings::NativeFormat, QSettings::UserScope, config.path());
    QSettings settings("Kog", "Kog");
    const QRect initial(55, 65, 420, 260);
    const QRect moved(100, 130, 500, 320);
    settings.setValue("MainWindow/normalGeometry", initial);
    settings.sync();

    {
        MainWindow window;
        kogRestoreMainWindow();
        QTest::qWait(250);
        require(window.geometry() == initial, "restored initial geometry");
        window.setGeometry(moved);
        window.close(); // Also cover closing before the debounce timeout.
        require(settings.value("MainWindow/normalGeometry").toRect() == moved, "saved moved geometry on close");
    }
    {
        MainWindow window;
        kogRestoreMainWindow();
        QTest::qWait(250);
        require(window.geometry() == moved, "restored moved geometry");
        window.showMinimized();
        QTest::qWait(250);
        window.hide();
        require(settings.value("MainWindow/normalGeometry").toRect() == moved, "minimize preserved normal geometry");
        require(!settings.value("MainWindow/maximized").toBool(), "minimize is not remembered as maximized");
        window.showNormal();
        QTest::qWait(250);
        window.showMaximized();
        QTest::qWait(250);
        require(settings.value("MainWindow/maximized").toBool(), "saved maximized state");
        require(settings.value("MainWindow/normalGeometry").toRect() == moved, "maximize preserved normal geometry");
        window.hide();
        require(window.property("restoreMaximized").toBool(), "tray restore keeps maximized state");
    }
    {
        MainWindow window;
        kogRestoreMainWindow();
        QTest::qWait(250);
        require(window.windowState() == Qt::WindowMaximized, "restored maximized window");
        window.close();
    }
    settings.setValue("MainWindow/maximized", false);
    settings.setValue("MainWindow/screen", "disconnected-display");
    settings.setValue("MainWindow/normalGeometry", QRect(-5000, 9000, 3000, 2000));
    settings.sync();
    {
        MainWindow window;
        kogRestoreMainWindow();
        QTest::qWait(250);
        require(window.screen()->availableGeometry().contains(window.geometry()), "recovered offscreen geometry");
        window.close();
    }
    {
        // A normal geometry covering almost the whole screen restores
        // invisibly, so it must be neither saved nor loaded (live report: a
        // persisted 3938x1618 normalGeometry on a 3938x1662 work area).
        settings.setValue("MainWindow/maximized", false);
        settings.setValue("MainWindow/normalGeometry", moved);
        settings.sync();
        MainWindow window;
        window.resize(500, 320);
        kogRestoreMainWindow();
        QTest::qWait(250);
        QScreen *screen = window.screen();
        if (!screen)
            screen = QGuiApplication::primaryScreen();
        require(screen, "test screen exists");
        const QRect nearlyFullscreen = screen->availableGeometry().adjusted(0, 0, 0, -44);
        window.setGeometry(nearlyFullscreen);
        window.close();
        require(settings.value("MainWindow/normalGeometry").toRect() != nearlyFullscreen,
            "near-fullscreen normal geometry is not saved");
        require(settings.value("MainWindow/normalGeometry").toRect() == moved,
            "sane normal geometry survives a near-fullscreen stint");
    }
    {
        QScreen *screen = QGuiApplication::primaryScreen();
        require(screen, "test screen exists");
        settings.setValue("MainWindow/maximized", false);
        settings.setValue("MainWindow/normalGeometry",
            screen->availableGeometry().adjusted(0, 0, 0, -44));
        settings.sync();
        MainWindow window;
        window.resize(500, 320);
        kogRestoreMainWindow();
        QTest::qWait(250);
        require(window.geometry().size() == QSize(500, 320),
            "poisoned near-fullscreen geometry falls back to declared size");
        window.close();
    }
    {
        QWindow popup;
        popup.setObjectName("kogNowPlayingNotification");
        kogRestoreMainWindow();
        require(!popup.isVisible(), "did not show or manage the notification window");
    }
    std::puts("Window geometry, maximize, tray, disconnected-screen, and near-fullscreen-normal tests passed");
}
