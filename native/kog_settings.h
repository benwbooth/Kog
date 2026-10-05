#pragma once
#include <QtCore/QSettings>
#include <QtCore/QString>
#include <QtCore/QVariant>
#include <functional>

// Qt keeps platform-specific QVariant encoding; the Rust port owns storage.
using KogSettingsPort = std::function<QString(const QString &, const QString &, bool)>;
void kogSetSettingsPort(KogSettingsPort port);
class KogSettings {
public:
    explicit KogSettings(const QString &group);
    QVariant value(const QString &key, const QVariant &fallback = {}) const;
    bool contains(const QString &key) const;
    void setValue(const QString &key, const QVariant &value);
private:
    QString m_group;
    QSettings m_legacy;
};
