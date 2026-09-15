#include "kog_cover_art_network.h"
#include <QtCore/QEventLoop>
#include <QtCore/QTimer>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkReply>
#include <QtNetwork/QNetworkRequest>
#include <stdexcept>

namespace {
bool allowed(const QUrl &url)
{
    if (url.scheme() != QStringLiteral("https") || !url.userInfo().isEmpty())
        return false;
    if (url.port() != -1 && url.port() != 443)
        return false;
    const QString host = url.host().toLower();
    return host == QStringLiteral("api.deezer.com")
        || host == QStringLiteral("cdn-images.dzcdn.net")
        || host == QStringLiteral("itunes.apple.com")
        || host.endsWith(QStringLiteral(".mzstatic.com"))
        || host == QStringLiteral("musicbrainz.org")
        || host == QStringLiteral("coverartarchive.org")
        || host == QStringLiteral("archive.org")
        || host.endsWith(QStringLiteral(".archive.org"))
        || host == QStringLiteral("duckduckgo.com");
}
} // namespace

// Runs on the cover-art worker. Never nests an event loop on the GUI thread.
QByteArray kogFetchCoverArtUrl(const QString &address, unsigned int maxBytes)
{
    const QUrl url(address);
    if (!allowed(url) || maxBytes > 16 * 1024 * 1024)
        throw std::runtime_error("Invalid cover art address or limit");
    QNetworkAccessManager manager;
    QNetworkRequest request(url);
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                         QNetworkRequest::UserVerifiedRedirectPolicy);
    request.setMaximumRedirectsAllowed(5);
    request.setRawHeader("User-Agent", "Kog cover art (https://github.com/benwbooth/Kog)");
    auto *reply = manager.get(request);
    reply->setReadBufferSize(256 * 1024);
    QEventLoop loop;
    QTimer timeout;
    timeout.setSingleShot(true);
    QByteArray result;
    QString failure;
    QObject::connect(&timeout, &QTimer::timeout, &loop, [&] {
        failure = QStringLiteral("Cover art request timed out. Please try again.");
        reply->abort();
    });
    QObject::connect(reply, &QNetworkReply::redirected, &loop, [&](const QUrl &target) {
        if (allowed(reply->url().resolved(target))) reply->redirectAllowed();
        else { failure = QStringLiteral("Blocked cover art redirect outside cover providers"); reply->abort(); }
    });
    const auto drain = [&] {
        const auto bytes = reply->readAll();
        if (result.size() + bytes.size() > maxBytes) {
            failure = QStringLiteral("Cover art download exceeds its size limit");
            reply->abort();
        } else result.append(bytes);
    };
    QObject::connect(reply, &QIODevice::readyRead, &loop, drain);
    QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
    timeout.start(20000);
    loop.exec();
    drain();
    if (!failure.isEmpty()) throw std::runtime_error(failure.toStdString());
    if (reply->error() != QNetworkReply::NoError)
        throw std::runtime_error(reply->errorString().toStdString());
    const int status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    if (status != 200) throw std::runtime_error("Cover service returned an unsuccessful response");
    return result;
}
