#ifndef GLOBAL_MENU_H
#define GLOBAL_MENU_H

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QVariant>
#include <QWindow>

class QPlatformMenu;
class QPlatformMenuBar;
class QPlatformMenuItem;

// The window's menus in the desktop's global menu bar (hearth's top bar,
// Plasma's Global Menu).
//
// Qt Quick's MenuBar only goes native on macOS (QQuickMenuBarPrivate::
// useNativeMenuBar() is false elsewhere), so this builds the platform theme's
// menu bar directly, as QMenuBar does for widget apps. Under the KDE platform
// theme that is a dbusmenu exported on the session bus, with its address set
// on the window through the KDE appmenu protocol (arbor reports it as the
// window's `app_menu`; X11 gets the _KDE_NET_WM_APPMENU properties). Without
// a global menu (no com.canonical.AppMenu.Registrar) the theme returns no
// menu bar and `available` stays false.
//
// `model` is a list of menus: { title, icon, items: [ { id, text, enabled,
// separator, shortcut, checkable, checked, icon, items (a submenu) } ] }.
// `shortcut` is a QKeySequence string ("Ctrl+Shift+N"): the menu bar only
// shows it, the app keeps handling the key itself. `icon` is a theme icon
// name. Ids must be unique across the whole model (items are updated in
// place by id). Choosing an item emits triggered(id).
//
// Shared by Ben's Qt Quick apps (copied from nixos-update-checker; keep the
// copies the same).
class GlobalMenuBar : public QObject {
    Q_OBJECT
    Q_PROPERTY(QWindow *window READ window WRITE setWindow NOTIFY windowChanged)
    Q_PROPERTY(QVariant model READ model WRITE setModel NOTIFY modelChanged)
    Q_PROPERTY(bool available READ available NOTIFY availableChanged)

public:
    explicit GlobalMenuBar(QObject *parent = nullptr);
    ~GlobalMenuBar() override;

    QWindow *window() const { return m_window; }
    void setWindow(QWindow *window);
    QVariant model() const { return m_model; }
    void setModel(const QVariant &model);
    bool available() const { return m_bar != nullptr; }

    // Re-announce the menu on the window (its surface exists only while it
    // is shown).
    Q_INVOKABLE void attach();

signals:
    void windowChanged();
    void modelChanged();
    void availableChanged();
    void triggered(const QString &id);

private:
    void rebuild();
    bool updateInPlace();
    void clear();
    void fill(QPlatformMenu *menu, const QVariantList &items);
    static void apply(QPlatformMenuItem *it, const QVariantMap &item);
    static QString structure(const QVariant &model);

    QPointer<QWindow> m_window;
    QVariant m_model;
    QString m_structure;
    QPlatformMenuBar *m_bar = nullptr;
    QList<QPlatformMenu *> m_menus;  // top-level and submenus, owned here
    QHash<QString, QPlatformMenuItem *> m_items;
    QHash<QString, QPlatformMenu *> m_itemMenu;  // item id -> menu holding it
};

#endif // GLOBAL_MENU_H
