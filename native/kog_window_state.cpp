#include "kog_desktop_integration.h"

#include <QtCore/QEvent>
#include <QtCore/QSettings>
#include <QtCore/QTimer>
#include <QtGui/QScreen>
#include <QtGui/QWindow>

#if defined(KOG_WAYLAND_SESSION_RESTORE) && QT_CONFIG(wayland)
#include <qpa/qplatformwindow_p.h>
#endif

#include <algorithm>

namespace {

class MainWindowState final : public QObject {
public:
    explicit MainWindowState(QWindow *window)
        : QObject(window), m_window(window), m_settings("Kog", "Kog")
    {
        m_settings.beginGroup("MainWindow");
        const QString screenName = m_settings.value("screen").toString();
        QScreen *screen = window->screen();
        for (QScreen *candidate : QGuiApplication::screens()) {
            if (candidate->name() == screenName) {
                screen = candidate;
                break;
            }
        }
        if (!screen)
            screen = QGuiApplication::primaryScreen();
        if (!screen)
            return;

        window->setScreen(screen);
        const QRect available = screen->availableGeometry();
        m_normal = m_settings.value("normalGeometry", window->geometry()).toRect();
        if (!m_normal.isValid())
            m_normal = window->geometry();
        m_normal.setSize(QSize(
            std::clamp(m_normal.width(), std::min(window->minimumWidth(), available.width()), available.width()),
            std::clamp(m_normal.height(), std::min(window->minimumHeight(), available.height()), available.height())));
        m_normal.moveLeft(std::clamp(m_normal.x(), available.left(), available.right() - m_normal.width() + 1));
        m_normal.moveTop(std::clamp(m_normal.y(), available.top(), available.bottom() - m_normal.height() + 1));
        m_maximized = m_settings.value("maximized", false).toBool();
        window->resize(m_normal.size());
        // Wayland positions come from the compositor's persistent session.
        // Normal setPosition() remains correct for Windows, macOS and X11.
        if (!QGuiApplication::platformName().startsWith(QStringLiteral("wayland"))
            && m_settings.contains("normalGeometry"))
            window->setPosition(m_normal.topLeft());

#if defined(KOG_WAYLAND_SESSION_RESTORE) && QT_CONFIG(wayland)
        if (QGuiApplication::platformName().startsWith(QStringLiteral("wayland"))) {
            // Create the platform window, but assign the role before show()
            // creates its xdg_toplevel. Only the main window joins this session.
            window->create();
            if (auto *native = window->nativeInterface<QNativeInterface::Private::QWaylandWindow>())
                native->setSessionRestoreId(QStringLiteral("kog-main"));
        }
#endif

        window->setProperty("restoreMaximized", m_maximized);
        m_timer.setSingleShot(true);
        m_timer.setInterval(200);
        connect(&m_timer, &QTimer::timeout, this, [this] { save(); });
        const auto schedule = [this] { m_timer.start(); };
        connect(window, &QWindow::xChanged, this, schedule);
        connect(window, &QWindow::yChanged, this, schedule);
        connect(window, &QWindow::widthChanged, this, schedule);
        connect(window, &QWindow::heightChanged, this, schedule);
        connect(window, &QWindow::windowStateChanged, this, [this](Qt::WindowState state) {
            if (state == Qt::WindowMaximized || state == Qt::WindowNoState) {
                m_maximized = state == Qt::WindowMaximized;
                m_window->setProperty("restoreMaximized", m_maximized);
            }
            m_timer.start();
        });
        connect(window, &QWindow::screenChanged, this, schedule);
        connect(window, &QWindow::visibleChanged, this, [this](bool visible) {
            if (!visible)
                save();
        });
        connect(qApp, &QCoreApplication::aboutToQuit, this, [this] { save(); });
        window->installEventFilter(this);
        if (m_maximized)
            window->showMaximized();
        else
            window->showNormal();
    }

private:
    bool eventFilter(QObject *object, QEvent *event) override
    {
        if (event->type() == QEvent::Close)
            save();
        return QObject::eventFilter(object, event);
    }

    void save()
    {
        m_timer.stop();
        if (m_window->windowState() == Qt::WindowNoState && !m_maximized)
            m_normal = m_window->geometry();
        m_settings.setValue("normalGeometry", m_normal);
        m_settings.setValue("maximized", m_maximized);
        if (m_window->screen())
            m_settings.setValue("screen", m_window->screen()->name());
        m_settings.sync();
    }

    QWindow *m_window;
    QSettings m_settings;
    QTimer m_timer;
    QRect m_normal;
    bool m_maximized = false;
};

} // namespace

void kogRestoreMainWindow()
{
    for (QWindow *window : QGuiApplication::allWindows()) {
        if (window->objectName() == QStringLiteral("kogMainWindow")) {
            new MainWindowState(window);
            return;
        }
    }
}
