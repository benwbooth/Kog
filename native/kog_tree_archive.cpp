#include "kog_tree_archive.h"

#include <archive.h>
#include <archive_entry.h>
#include <QtCore/QCache>
#include <QtCore/QDateTime>
#include <QtCore/QFileInfo>
#include <QtCore/QFile>
#include <QtCore/QMutex>
#include <QtCore/QSet>
#include <QtCore/QUrl>
#include <QtCore/QUrlQuery>

namespace {
std::function<QString(const QByteArray &)> decodeName = [](const QByteArray &bytes) {
    return QString::fromUtf8(bytes);
};
QMutex cacheMutex;
QCache<QString, KogArchiveListing> cache(32768);
QString safeName(QString name)
{
    name.replace('\\', '/');
    if (name.startsWith('/') || (name.size() >= 2 && name[1] == ':')) return {};
    QStringList parts;
    for (const auto &part : name.split('/', Qt::SkipEmptyParts)) {
        if (part == "..") return {};
        if (part != ".") parts.append(part);
    }
    return parts.join('/');
}
}

void kogSetArchiveNameDecoder(std::function<QString(const QByteArray &)> decoder)
{
    decodeName = std::move(decoder);
}

bool kogIsArchive(const QString &path)
{
    if (path.startsWith("kog-archive:")) return false; // Nested archives are not playable yet.
    static const QSet<QString> extensions {
        "zip", "rar", "7z", "rsn", "vgm7z", "gz", "mdz", "mdr", "s3z", "xmz", "itz", "mptmz"
    };
    const QFileInfo info(path);
    return extensions.contains(info.suffix().toLower()) && !info.isDir();
}

QString kogArchiveUrl(const QString &archive, const QString &entry, bool directory)
{
    return QStringLiteral("kog-archive:?archive=%1&entry=%2&directory=%3")
        .arg(QString::fromLatin1(QUrl::toPercentEncoding(archive)),
             QString::fromLatin1(QUrl::toPercentEncoding(entry)), directory ? "1" : "0");
}

KogArchiveLocation kogArchiveLocation(const QString &url)
{
    if (!url.startsWith("kog-archive:")) return {};
    const QUrlQuery query{QUrl(url)};
    return {query.queryItemValue("archive", QUrl::FullyDecoded),
            query.queryItemValue("entry", QUrl::FullyDecoded),
            query.queryItemValue("directory") == "1"};
}

KogArchiveListing kogListArchive(const QString &path,
    const std::shared_ptr<std::atomic_bool> &cancel)
{
    const QFileInfo info(path);
    const auto key = path + '\n' + QString::number(info.size()) + '\n'
        + QString::number(info.lastModified().toMSecsSinceEpoch());
    {
        QMutexLocker lock(&cacheMutex);
        if (auto *hit = cache.object(key)) return *hit;
    }
    KogArchiveListing result;
    std::unique_ptr<struct archive, decltype(&archive_read_free)> reader(archive_read_new(), archive_read_free);
    archive_read_support_filter_all(reader.get());
    archive_read_support_format_zip(reader.get());
    archive_read_support_format_7zip(reader.get());
    archive_read_support_format_rar(reader.get());
    archive_read_support_format_rar5(reader.get());
    archive_read_support_format_tar(reader.get());
    if (info.suffix().compare("gz", Qt::CaseInsensitive) == 0)
        archive_read_support_format_raw(reader.get());
#ifdef Q_OS_WIN
    const auto native = path.toStdWString();
    int status = archive_read_open_filename_w(reader.get(), native.c_str(), 10240);
#else
    const auto native = QFile::encodeName(path);
    int status = archive_read_open_filename(reader.get(), native.constData(), 10240);
#endif
    int count = 0;
    qint64 total = 0;
    struct archive_entry *entry = nullptr;
    while (status >= ARCHIVE_WARN && !cancel->load()) {
        status = archive_read_next_header(reader.get(), &entry);
        if (status == ARCHIVE_EOF) break;
        if (status < ARCHIVE_WARN) break;
        const qint64 size = archive_entry_size(entry);
        if (++count > 16384 || size > 4LL * 1024 * 1024 * 1024
            || (total += qMax<qint64>(0, size)) > 8LL * 1024 * 1024 * 1024) {
            result.error = QStringLiteral("Archive exceeds Kog's entry or expanded-size safety limit");
            break;
        }
        const char *raw = archive_entry_pathname(entry);
        QString name = raw ? decodeName(QByteArray(raw)) : QString();
        if (name.size() > 4096) {
            result.error = QStringLiteral("Archive entry name exceeds the browsing safety limit");
            break;
        }
        if (archive_format(reader.get()) == ARCHIVE_FORMAT_RAW && name == "data")
            name = info.completeBaseName();
        name = safeName(name);
        const auto type = archive_entry_filetype(entry);
        if (!name.isEmpty() && !archive_entry_hardlink(entry)
            && (type == AE_IFREG || type == AE_IFDIR || type == 0)) {
            if (!result.entries.contains(name)) result.entries.insert(name, type == AE_IFDIR);
            auto parent = QFileInfo(name).path();
            while (parent != "." && !parent.isEmpty()) {
                result.entries.insert(parent, true);
                parent = QFileInfo(parent).path();
            }
            if (result.entries.size() > 32768) {
                result.error = QStringLiteral("Archive directory index exceeds the browsing safety limit");
                break;
            }
        }
        // A raw gzip stream has only one member. Its name is enough for the
        // index; skipping its data would unnecessarily decompress the payload.
        if (archive_format(reader.get()) == ARCHIVE_FORMAT_RAW) { status = ARCHIVE_EOF; break; }
        status = archive_read_data_skip(reader.get());
    }
    if (status < ARCHIVE_WARN && result.error.isEmpty())
        result.error = QString::fromUtf8(archive_error_string(reader.get()));
    if (!result.error.isEmpty()) result.entries.clear();
    if (!cancel->load() && result.error.isEmpty()) {
        QMutexLocker lock(&cacheMutex);
        cache.insert(key, new KogArchiveListing(result), qMax(1, int(result.entries.size())));
    }
    return result;
}
