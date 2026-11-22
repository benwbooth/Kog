#ifndef GLOBAL_MENU_FALLBACK_H
#define GLOBAL_MENU_FALLBACK_H

#include <QObject>
#include <QPointer>
#include <QVariant>
#include <QWindow>

// Stands in for GlobalMenuBar (global_menu.h) in builds whose Qt has no
// private QPA headers (build.rs decides): the same QML API, never available,
// so the menus are only in the window's ☰ button.
class GlobalMenuBarFallback : public QObject {
    Q_OBJECT
    Q_PROPERTY(QWindow *window READ window WRITE setWindow NOTIFY windowChanged)
    Q_PROPERTY(QVariant model READ model WRITE setModel NOTIFY modelChanged)
    Q_PROPERTY(bool available READ available NOTIFY availableChanged)

public:
    explicit GlobalMenuBarFallback(QObject *parent = nullptr) : QObject(parent) {}

    QWindow *window() const { return m_window; }
    void setWindow(QWindow *window) {
        if (m_window == window) return;
        m_window = window;
        emit windowChanged();
    }
    QVariant model() const { return m_model; }
    void setModel(const QVariant &model) {
        m_model = model;
        emit modelChanged();
    }
    bool available() const { return false; }
    Q_INVOKABLE void attach() {}

signals:
    void windowChanged();
    void modelChanged();
    void availableChanged();
    void triggered(const QString &id);

private:
    QPointer<QWindow> m_window;
    QVariant m_model;
};

#endif // GLOBAL_MENU_FALLBACK_H
