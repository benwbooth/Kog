#include "kog_skin_network.h"
#include <QtCore/QEventLoop>
#include <QtCore/QTimer>
#include <QtCore/QHash>
#include <QtCore/QFile>
#include <QtCore/QXmlStreamReader>
#include <QtGui/QImageReader>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkReply>
#include <QtNetwork/QNetworkRequest>
#include <stdexcept>

static bool allowed(const QUrl &url)
{
    return url.scheme() == QStringLiteral("https") && url.userInfo().isEmpty()
        && (url.port() == -1 || url.port() == 443)
        && (url.host() == QStringLiteral("archive.org")
            || url.host().endsWith(QStringLiteral(".archive.org")));
}

bool kogValidateModernSkin(const QString &path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly) || file.size() > 2 * 1024 * 1024) return false;
    QXmlStreamReader xml(&file);
    bool root = false;
    while (!xml.atEnd()) {
        const auto token = xml.readNext();
        if (token == QXmlStreamReader::DTD) return false;
        if (!root && token == QXmlStreamReader::StartElement) {
            if (xml.name().compare(QStringLiteral("WinampAbstractionLayer"), Qt::CaseInsensitive) != 0
                && xml.name().compare(QStringLiteral("WasabiXML"), Qt::CaseInsensitive) != 0) return false;
            root = true;
        }
    }
    return root && !xml.hasError();
}

bool kogValidateModernImage(const QString &path)
{
    QImageReader reader(path);
    const auto size = reader.size();
    if (size.width() <= 0 || size.height() <= 0 || size.width() > 16384 || size.height() > 16384
        || qint64(size.width()) * size.height() > 16 * 1024 * 1024) return false;
    return !reader.read().isNull();
}

// Runs on the import worker. Never nests an event loop on the GUI thread.
QByteArray kogFetchSkinUrl(const QString &address, unsigned int maxBytes)
{
    const QUrl url(address);
    if (!allowed(url) || maxBytes > 32 * 1024 * 1024)
        throw std::runtime_error("Invalid skin download address or limit");
    QNetworkAccessManager manager;
    QNetworkRequest request(url);
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                         QNetworkRequest::UserVerifiedRedirectPolicy);
    request.setMaximumRedirectsAllowed(5);
    request.setRawHeader("User-Agent", "Kog skin gallery/1.0");
    auto *reply = manager.get(request);
    reply->setReadBufferSize(256 * 1024);
    QEventLoop loop;
    QTimer timeout;
    timeout.setSingleShot(true);
    QByteArray result;
    QString failure;
    QObject::connect(&timeout, &QTimer::timeout, &loop, [&] {
        failure = QStringLiteral("Internet Archive request timed out. Please try again.");
        reply->abort();
    });
    QObject::connect(reply, &QNetworkReply::redirected, &loop, [&](const QUrl &target) {
        if (allowed(reply->url().resolved(target))) reply->redirectAllowed();
        else { failure = QStringLiteral("Blocked download redirect outside Internet Archive"); reply->abort(); }
    });
    const auto drain = [&] {
        const auto bytes = reply->readAll();
        if (result.size() + bytes.size() > maxBytes) {
            failure = QStringLiteral("Skin download exceeds its size limit");
            reply->abort();
        } else result.append(bytes);
    };
    QObject::connect(reply, &QIODevice::readyRead, &loop, drain);
    QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
    timeout.start(30000);
    loop.exec();
    drain();
    if (!failure.isEmpty()) throw std::runtime_error(failure.toStdString());
    if (reply->error() != QNetworkReply::NoError)
        throw std::runtime_error(reply->errorString().toStdString());
    const int status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    if (status != 200) throw std::runtime_error("Internet Archive returned an unsuccessful response");
    return result;
}

bool kogValidateSkinImage(const QString &path, unsigned int minWidth, unsigned int minHeight)
{
    QImageReader reader(path, "bmp");
    const QSize size = reader.size();
    if (size.width() < int(minWidth) || size.height() < int(minHeight)
        || size.width() > 4096 || size.height() > 4096
        || qint64(size.width()) * size.height() > 4 * 1024 * 1024)
        return false;
    return !reader.read().isNull();
}

QString kogSkinTextColors(const QString &path)
{
    // Caller has validated the bitmap's dimensions and decoded it successfully.
    const QImage image(path, "bmp");
    QHash<QRgb, int> colors;
    for (int y = 0; y < qMin(12, image.height()); ++y)
        for (int x = 0; x < qMin(150, image.width()); ++x)
            ++colors[image.pixel(x, y)];
    const auto mostFrequent = [&colors](QRgb fallback) {
        QRgb best = fallback;
        int count = 0;
        for (auto i = colors.cbegin(); i != colors.cend(); ++i)
            if (i.value() > count) { best = i.key(); count = i.value(); }
        return best;
    };
    const QRgb background = mostFrequent(qRgb(0, 0, 0));
    colors.remove(background);
    const QRgb foreground = mostFrequent(qRgb(113, 245, 176));
    return QColor(background).name() + QLatin1Char(',') + QColor(foreground).name();
}
