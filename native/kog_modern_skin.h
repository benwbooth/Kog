#pragma once

#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtCore/QUrl>

bool kogModernRequestAllowed(const QUrl &url);
void kogInitializeModernSkins();
void kogRegisterModernSkinTypes();

// Each modern player gets an off-the-record profile with no filesystem or
// network access. Only its already-validated skin archive is served to it.
class KogModernProfile : public QQuickWebEngineProfile {
    Q_OBJECT
    Q_PROPERTY(QString skinPath READ skinPath WRITE setSkinPath NOTIFY skinPathChanged)
public:
    explicit KogModernProfile(QObject *parent = nullptr);
    QString skinPath() const { return m_skinPath; }
    void setSkinPath(const QString &path);
signals:
    void skinPathChanged();
private:
    QString m_skinPath;
};
