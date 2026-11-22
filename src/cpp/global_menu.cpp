#include "global_menu.h"

#include <functional>
#include <QDebug>
#include <QGuiApplication>
#include <QIcon>
#include <QKeySequence>
#include <QPointer>
#include <QStringList>
#include <qpa/qplatformmenu.h>
#include <qpa/qplatformtheme.h>
#include <private/qguiapplication_p.h>

GlobalMenuBar::GlobalMenuBar(QObject *parent) : QObject(parent) {
    if (QPlatformTheme *theme = QGuiApplicationPrivate::platformTheme()) {
        m_bar = theme->createPlatformMenuBar();
    }
}

GlobalMenuBar::~GlobalMenuBar() {
    if (m_bar) m_bar->handleReparent(nullptr);
    clear();
    delete m_bar;
}

void GlobalMenuBar::setWindow(QWindow *window) {
    if (m_window == window) return;
    m_window = window;
    attach();
    emit windowChanged();
}

void GlobalMenuBar::attach() {
    if (!m_bar || !m_window) return;
    // No global menu bar to serve without a real display (offscreen tests
    // crashed in QDBusMenuBar::registerMenuBar), and none before the
    // window has its native surface: attach once it's shown.
    if (QGuiApplication::platformName() == QLatin1String("offscreen")) return;
    if (!m_window->handle()) {
        QPointer<QWindow> w = m_window;
        auto *c = new QMetaObject::Connection;
        *c = connect(m_window, &QWindow::visibleChanged, this, [this, w, c](bool on) {
            if (!on || !w || w != m_window || !w->handle()) return;
            disconnect(*c);
            delete c;
            attach();
        });
        return;
    }
    // Reparenting to the same window is a no-op in the theme, so detach
    // first: the window may have a new surface since the last time.
    m_bar->handleReparent(nullptr);
    m_bar->handleReparent(m_window);
}

void GlobalMenuBar::setModel(const QVariant &model) {
    m_model = model;
    if (!updateInPlace()) rebuild();
    emit modelChanged();
}

// Ids and nesting, without texts or states: when only those change, the
// existing items are updated instead of rebuilding the menus.
QString GlobalMenuBar::structure(const QVariant &model) {
    QStringList parts;
    std::function<void(const QVariantList &)> walk = [&](const QVariantList &items) {
        parts << QStringLiteral("[");
        for (const QVariant &v : items) {
            const QVariantMap item = v.toMap();
            parts << (item.value("separator").toBool() ? QStringLiteral("-") : item.value("id").toString());
            if (item.contains("items")) walk(item.value("items").toList());
        }
        parts << QStringLiteral("]");
    };
    for (const QVariant &m : model.toList()) {
        const QVariantMap menu = m.toMap();
        parts << menu.value("title").toString();
        walk(menu.value("items").toList());
    }
    return parts.join('\x1f');
}

bool GlobalMenuBar::updateInPlace() {
    if (!m_bar || m_menus.isEmpty() || structure(m_model) != m_structure) return false;
    std::function<void(const QVariantList &)> walk = [&](const QVariantList &items) {
        for (const QVariant &v : items) {
            const QVariantMap item = v.toMap();
            const QString id = item.value("id").toString();
            if (QPlatformMenuItem *it = m_items.value(id)) {
                apply(it, item);
                if (QPlatformMenu *menu = m_itemMenu.value(id)) menu->syncMenuItem(it);
            }
            if (item.contains("items")) walk(item.value("items").toList());
        }
    };
    for (const QVariant &m : m_model.toList()) walk(m.toMap().value("items").toList());
    return true;
}

void GlobalMenuBar::apply(QPlatformMenuItem *it, const QVariantMap &item) {
    it->setText(item.value("text").toString());
    it->setEnabled(item.value("enabled", true).toBool());
    it->setShortcut(QKeySequence(item.value("shortcut").toString(), QKeySequence::PortableText));
    const bool checkable = item.value("checkable").toBool();
    it->setCheckable(checkable);
    it->setChecked(checkable && item.value("checked").toBool());
    const QString icon = item.value("icon").toString();
    it->setIcon(icon.isEmpty() ? QIcon() : QIcon::fromTheme(icon));
}

void GlobalMenuBar::clear() {
    for (QPlatformMenu *menu : std::as_const(m_menus)) {
        if (m_bar && m_bar->menuForTag(reinterpret_cast<quintptr>(menu)) == menu) {
            m_bar->removeMenu(menu);
        }
    }
    qDeleteAll(m_items);
    m_items.clear();
    m_itemMenu.clear();
    qDeleteAll(m_menus);
    m_menus.clear();
}

void GlobalMenuBar::fill(QPlatformMenu *menu, const QVariantList &items) {
    for (const QVariant &v : items) {
        const QVariantMap item = v.toMap();
        QPlatformMenuItem *it = menu->createMenuItem();
        if (!it) continue;
        const QString id = item.value("id").toString();
        it->setTag(reinterpret_cast<quintptr>(it));
        if (item.value("separator").toBool()) {
            it->setIsSeparator(true);
        } else {
            apply(it, item);
            if (item.contains("items")) {
                QPlatformMenu *sub = menu->createSubMenu();
                if (!sub && m_bar) sub = m_bar->createMenu();
                if (sub) {
                    sub->setTag(reinterpret_cast<quintptr>(sub));
                    sub->setText(item.value("text").toString());
                    const QString subIcon = item.value("icon").toString();
                    if (!subIcon.isEmpty()) sub->setIcon(QIcon::fromTheme(subIcon));
                    sub->setEnabled(true);
                    sub->setVisible(true);
                    fill(sub, item.value("items").toList());
                    it->setMenu(sub);
                    m_menus << sub;
                }
            } else if (!id.isEmpty()) {
                connect(it, &QPlatformMenuItem::activated, this, [this, id] { emit triggered(id); });
            }
        }
        it->setVisible(true);
        menu->insertMenuItem(it, nullptr);
        if (!id.isEmpty()) {
            if (m_items.contains(id)) qWarning() << "GlobalMenuBar: duplicate menu id" << id;
            m_items.insert(id, it);
            m_itemMenu.insert(id, menu);
        } else {
            // Separators: owned through a generated key.
            m_items.insert(QStringLiteral("\x1f%1").arg(reinterpret_cast<quintptr>(it)), it);
        }
    }
}

void GlobalMenuBar::rebuild() {
    if (!m_bar) return;
    clear();
    for (const QVariant &m : m_model.toList()) {
        const QVariantMap menuDef = m.toMap();
        // The menu bar's own kind of menu (a dbusmenu one under the KDE
        // theme), as QMenuBar asks for; the theme's general
        // createPlatformMenu() gives none.
        QPlatformMenu *menu = m_bar->createMenu();
        if (!menu) continue;
        menu->setTag(reinterpret_cast<quintptr>(menu));
        menu->setText(menuDef.value("title").toString());
        // The menu's own icon (shown by menu bars that draw title icons,
        // e.g. hearth): the dbusmenu export puts it on the title's item.
        const QString icon = menuDef.value("icon").toString();
        if (!icon.isEmpty()) menu->setIcon(QIcon::fromTheme(icon));
        menu->setEnabled(true);
        menu->setVisible(true);
        fill(menu, menuDef.value("items").toList());
        m_bar->insertMenu(menu, nullptr);
        m_bar->syncMenu(menu);
        m_menus << menu;
    }
    m_structure = structure(m_model);
}
