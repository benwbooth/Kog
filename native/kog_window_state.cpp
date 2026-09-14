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

QScreen *geometryScreen(const QWindow *window)
{
    if (QScreen *screen = window->screen())
        return screen;
    return QGuiApplication::primaryScreen();
}

// A normal-state geometry that covers (almost) the whole screen is
// indistinguishable from maximized, so restoring to it looks like a no-op
// (seen live: a persisted 3938x1618 normalGeometry on a 3938x1662 work
// area). Never persist such a geometry, and fall back to the declared
// window size when loading one. The margin catches tiled and edge-filled
// windows whose frames sit a panel-height shy of the work area.
bool coversAvailableScreen(const QWindow *window, const QRect &geometry)
{
    QScreen *screen = geometryScreen(window);
    if (!screen || geometry.isEmpty())
        return false;
    const QSize available = screen->availableGeometry().size();
    if (available.isEmpty())
        return false;
    return geometry.width() >= available.width() * 0.9
        && geometry.height() >= available.height() * 0.9;
}

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
        if (coversAvailableScreen(window, m_normal)) {
            QSize fallback = window->geometry().size();
            if (fallback.isEmpty())
                fallback = window->minimumSize();
            if (!fallback.isEmpty()) {
                m_normal.setSize(QSize(
                    std::clamp(fallback.width(), std::min(window->minimumWidth(), available.width()), available.width()),
                    std::clamp(fallback.height(), std::min(window->minimumHeight(), available.height()), available.height())));
                m_normal.moveCenter(available.center());
            }
        }
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
        if (m_window->windowState() == Qt::WindowNoState && !m_maximized
            && !coversAvailableScreen(m_window, m_window->geometry()))
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
