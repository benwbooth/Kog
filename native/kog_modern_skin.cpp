#include "kog_modern_skin.h"

#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtQml/qqml.h>
#include <QtWebEngineCore/QWebEngineUrlRequestInfo>
#include <QtWebEngineCore/QWebEngineUrlRequestInterceptor>
#include <QtWebEngineCore/QWebEngineUrlRequestJob>
#include <QtWebEngineCore/QWebEngineUrlScheme>
#include <QtWebEngineCore/QWebEngineUrlSchemeHandler>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

bool kogModernRequestAllowed(const QUrl &url)
{
    if (!url.isValid() || !url.userInfo().isEmpty()) return false;
    if (url.scheme() == "blob" || url.scheme() == "data") return true;
    if (url.scheme() == "kogskin")
        return url.host() == "current" && url.port() == -1 && url.path() == "/skin.wal";
    if (url.scheme() == "qrc" && url.host().isEmpty()) {
        const auto path = url.adjusted(QUrl::NormalizePathSegments).path();
        return path.startsWith("/kog/modern/") || path == "/qtwebchannel/qwebchannel.js";
    }
    return false;
}

namespace {
class LocalOnly final : public QWebEngineUrlRequestInterceptor {
public:
    using QWebEngineUrlRequestInterceptor::QWebEngineUrlRequestInterceptor;
    void interceptRequest(QWebEngineUrlRequestInfo &info) override {
        info.block(!kogModernRequestAllowed(info.requestUrl()));
    }
};
class SkinArchive final : public QWebEngineUrlSchemeHandler {
public:
    explicit SkinArchive(KogModernProfile *profile) : QWebEngineUrlSchemeHandler(profile), m_profile(profile) {}
    void requestStarted(QWebEngineUrlRequestJob *job) override {
        if (job->requestMethod() != "GET" || !kogModernRequestAllowed(job->requestUrl())) {
            job->fail(QWebEngineUrlRequestJob::RequestDenied);
            return;
        }
        const auto path = m_profile->skinPath();
        const QFileInfo info(path);
        if (path.isEmpty() || !info.isFile() || info.isSymLink() || info.size() > 32 * 1024 * 1024) {
            job->fail(QWebEngineUrlRequestJob::UrlNotFound);
            return;
        }
        auto *file = new QFile(path, job);
        if (!file->open(QIODevice::ReadOnly)) { job->fail(QWebEngineUrlRequestJob::RequestFailed); return; }
        job->reply("application/zip", file);
    }
private:
    KogModernProfile *m_profile;
};
}

KogModernProfile::KogModernProfile(QObject *parent) : QQuickWebEngineProfile(parent)
{
    setUrlRequestInterceptor(new LocalOnly(this));
    installUrlSchemeHandler("kogskin", new SkinArchive(this));
}

void KogModernProfile::setSkinPath(const QString &path)
{
    if (m_skinPath == path) return;
    m_skinPath = path;
    emit skinPathChanged();
}

void kogInitializeModernSkins()
{
    QWebEngineUrlScheme scheme("kogskin");
    scheme.setSyntax(QWebEngineUrlScheme::Syntax::Host);
    auto flags = QWebEngineUrlScheme::SecureScheme | QWebEngineUrlScheme::CorsEnabled;
#if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
    flags |= QWebEngineUrlScheme::FetchApiAllowed;
#endif
    scheme.setFlags(flags);
    QWebEngineUrlScheme::registerScheme(scheme);
    QtWebEngineQuick::initialize();
}

void kogRegisterModernSkinTypes()
{
    qmlRegisterType<KogModernProfile>("org.kog.native", 1, 0, "ModernSkinProfile");
}
