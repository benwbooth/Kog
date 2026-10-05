#include "kog_settings.h"
#include <QtCore/QDataStream>
#include <QtCore/QIODevice>
#include <QtCore/QDebug>

namespace {
KogSettingsPort port;
QString encode(const QVariant &value)
{
    QByteArray bytes;
    QDataStream stream(&bytes, QIODevice::WriteOnly);
    stream.setVersion(QDataStream::Qt_6_0);
    stream << value;
    return QString::fromLatin1(bytes.toBase64());
}
QString request(const QString &key, const QString &value, bool write)
{
    if (!port) { qWarning() << "Kog settings storage was not configured"; return {}; }
    return port(key, value, write);
}
}

void kogSetSettingsPort(KogSettingsPort value) { port = std::move(value); }
KogSettings::KogSettings(const QString &group) : m_group(group), m_legacy("Kog", "Kog") {}
QVariant KogSettings::value(const QString &key, const QVariant &fallback) const
{
    const QString legacyKey = m_group + "/" + key;
    const QString saved = request("qt-native/" + legacyKey,
        m_legacy.contains(legacyKey) ? encode(m_legacy.value(legacyKey)) : QString(), false);
    if (saved.isEmpty()) return fallback;
    QByteArray bytes = QByteArray::fromBase64(saved.toLatin1());
    QDataStream stream(&bytes, QIODevice::ReadOnly);
    stream.setVersion(QDataStream::Qt_6_0);
    QVariant result;
    stream >> result;
    return stream.status() == QDataStream::Ok ? result : fallback;
}
bool KogSettings::contains(const QString &key) const { return value(key).isValid(); }
void KogSettings::setValue(const QString &key, const QVariant &value)
{
    request("qt-native/" + m_group + "/" + key, encode(value), true);
}
