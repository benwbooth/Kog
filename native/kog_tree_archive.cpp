#include "kog_tree_archive.h"
#include "kog_media_path.h"

#include <archive.h>
#include <archive_entry.h>
#include <QtCore/QCache>
#include <QtCore/QDateTime>
#include <QtCore/QFileInfo>
#include <QtCore/QFile>
#include <QtCore/QMutex>
#include <QtCore/QSet>
#include <QtCore/QTemporaryFile>
#include <QtCore/QUrl>
#include <QtCore/QUrlQuery>
#include <QtCore/QCryptographicHash>
#include <QtCore/QDir>
#include <vector>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QStandardPaths>

namespace {
std::function<QString(const QByteArray &)> decodeName = [](const QByteArray &bytes) {
    return QString::fromUtf8(bytes);
};
const QSet<QString> &archiveExtensions()
{
    static const QSet<QString> extensions {
        "zip", "rar", "7z", "rsn", "vgm7z", "gz", "mdz", "mdr", "s3z", "xmz", "itz", "mptmz"
    };
    return extensions;
}
constexpr int maxNestedDepth = 4;
constexpr qint64 maxNestedMemberBytes = 512LL * 1024 * 1024;
QMutex cacheMutex;
QString safeName(QString name);
#ifdef KOG_TREE_TESTS
std::function<void()> beforeRead;
#endif
// Bound by approximate bytes, not entry count: the old 32K-entry cache could
// evict the start of a music library before the next keystroke searched it.
QCache<QString, KogArchiveListing> cache(64 * 1024 * 1024);
int cacheCost(const KogArchiveListing &listing)
{
    int bytes = 256;
    for (auto it = listing.entries.cbegin(); it != listing.entries.cend(); ++it)
        bytes += 96 + int(it.key().size()) * 2;
    return bytes;
}
QString indexPath(const QString &path)
{
    const auto root = QStandardPaths::writableLocation(QStandardPaths::CacheLocation);
    if (root.isEmpty()) return {};
    const auto name = QCryptographicHash::hash(path.toUtf8(), QCryptographicHash::Sha256).toHex();
    return root + "/archive-index-v1/" + QString::fromLatin1(name) + ".json";
}
bool readIndex(const QString &fileName, const QString &key, KogArchiveListing &listing)
{
    QFile file(fileName);
    if (!file.open(QIODevice::ReadOnly) || file.size() > 8 * 1024 * 1024) return false;
    const auto object = QJsonDocument::fromJson(file.readAll()).object();
    if (object.value("fingerprint").toString() != key || !object.value("entries").isArray()) return false;
    const auto entries = object.value("entries").toArray();
    if (entries.size() > 32768) return false;
    for (const auto &entry : entries) {
        const auto pair = entry.toArray();
        if (pair.size() != 2 || !pair[0].isString() || !pair[1].isBool()) return false;
        const auto name = pair[0].toString();
        if (name.isEmpty() || name.size() > 4096 || safeName(name) != name) return false;
        if (!kogIsMetadataPath(name)) listing.entries.insert(name, pair[1].toBool());
    }
    listing.fromCache = true;
    return true;
}
void writeIndex(const QString &fileName, const QString &key, const KogArchiveListing &listing)
{
    if (fileName.isEmpty() || !QDir().mkpath(QFileInfo(fileName).path())) return;
    QJsonArray entries;
    for (auto it = listing.entries.cbegin(); it != listing.entries.cend(); ++it)
        entries.append(QJsonArray{it.key(), it.value()});
    const auto bytes = QJsonDocument(QJsonObject{{"fingerprint", key}, {"entries", entries}}).toJson(QJsonDocument::Compact);
    if (bytes.size() > 8 * 1024 * 1024) return;
    // This is a disposable index, not user data. QSaveFile::commit fsyncs each
    // archive and turned a sub-second scan into tens of seconds. An interrupted
    // cache write simply fails JSON validation and is rebuilt on the next read.
    QFile file(fileName);
    if (file.open(QIODevice::WriteOnly | QIODevice::Truncate)) file.write(bytes);
    file.close();
    // Cache files are disposable. Amortize housekeeping across writes and cap
    // the on-disk index so browsing a large library cannot grow it indefinitely.
    static std::atomic_uint writes{0};
    if (++writes % 32 == 0) {
        const auto files = QDir(QFileInfo(fileName).path()).entryInfoList({"*.json"}, QDir::Files, QDir::Time);
        qint64 total = 0;
        for (const auto &entry : files) {
            total += entry.size();
            if (total > 256LL * 1024 * 1024) QFile::remove(entry.absoluteFilePath());
        }
    }
}
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

void openListingReader(struct archive *reader, const QFileInfo &info)
{
    archive_read_support_filter_all(reader);
    archive_read_support_format_zip(reader);
    archive_read_support_format_7zip(reader);
    archive_read_support_format_rar(reader);
    archive_read_support_format_rar5(reader);
    archive_read_support_format_tar(reader);
    if (info.suffix().compare("gz", Qt::CaseInsensitive) == 0)
        archive_read_support_format_raw(reader);
}

// Shared listing loop for on-disk files and temporary nested materializations.
// Takes the open status in and returns the final status; caching stays with
// the caller so transient nested levels never pollute the index.
int readArchiveEntries(struct archive *reader, int status, const QFileInfo &info,
    KogArchiveListing &result, const std::shared_ptr<std::atomic_bool> &cancel)
{
    int count = 0;
    qint64 total = 0;
    struct archive_entry *entry = nullptr;
    while (status >= ARCHIVE_WARN && !cancel->load()) {
        status = archive_read_next_header(reader, &entry);
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
        if (archive_format(reader) == ARCHIVE_FORMAT_RAW && name == "data")
            name = info.completeBaseName();
        name = safeName(name);
        if (kogIsMetadataPath(name)) continue;
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
        if (archive_format(reader) == ARCHIVE_FORMAT_RAW) { status = ARCHIVE_EOF; break; }
        status = archive_read_data_skip(reader);
    }
    return status;
}

} // namespace

#ifdef KOG_TREE_TESTS
void kogClearArchiveMemoryCache() { QMutexLocker lock(&cacheMutex); cache.clear(); }
void kogSetArchiveReadTestHook(std::function<void()> hook) { beforeRead = std::move(hook); }
#endif

void kogSetArchiveNameDecoder(std::function<QString(const QByteArray &)> decoder)
{
    decodeName = std::move(decoder);
}

bool kogIsArchiveName(const QString &name)
{
    return archiveExtensions().contains(QFileInfo(name).suffix().toLower());
}

bool kogIsArchive(const QString &path)
{
    if (path.startsWith("kog-archive:")) {
        const QUrlQuery query{QUrl(path)};
        return kogIsArchiveName(query.queryItemValue("entry", QUrl::FullyDecoded));
    }
    const QFileInfo info(path);
    return archiveExtensions().contains(info.suffix().toLower()) && !info.isDir();
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
        if (auto *hit = cache.object(key)) { auto result = *hit; result.fromCache = true; return result; }
    }
    KogArchiveListing result;
    const auto diskIndex = indexPath(path);
    if (readIndex(diskIndex, key, result)) {
        QMutexLocker lock(&cacheMutex);
        cache.insert(key, new KogArchiveListing(result), cacheCost(result));
        return result;
    }
    result = {};
#ifdef KOG_TREE_TESTS
    if (beforeRead) beforeRead();
#endif
    std::unique_ptr<struct archive, decltype(&archive_read_free)> reader(archive_read_new(), archive_read_free);
    openListingReader(reader.get(), info);
#ifdef Q_OS_WIN
    const auto native = path.toStdWString();
    int status = archive_read_open_filename_w(reader.get(), native.c_str(), 10240);
#else
    const auto native = QFile::encodeName(path);
    int status = archive_read_open_filename(reader.get(), native.constData(), 10240);
#endif
    status = readArchiveEntries(reader.get(), status, info, result, cancel);
    if (status < ARCHIVE_WARN && result.error.isEmpty())
        result.error = QString::fromUtf8(archive_error_string(reader.get()));
    if (!result.error.isEmpty()) result.entries.clear();
    if (!cancel->load() && result.error.isEmpty()) {
        writeIndex(diskIndex, key, result);
        QMutexLocker lock(&cacheMutex);
        cache.insert(key, new KogArchiveListing(result), cacheCost(result));
    }
    return result;
}

namespace {
// Streams one member's full bytes out of an on-disk archive. The reader is
// opened fresh by the caller; iteration always starts at the first header.
QByteArray readNestedMember(struct archive *reader,
    const QString &diskFile, const QString &link, QString &error,
    const std::shared_ptr<std::atomic_bool> &cancel)
{
    const QFileInfo info(diskFile);
    openListingReader(reader, info);
#ifdef Q_OS_WIN
    const auto native = diskFile.toStdWString();
    int status = archive_read_open_filename_w(reader, native.c_str(), 10240);
#else
    const auto encoded = QFile::encodeName(diskFile);
    int status = archive_read_open_filename(reader, encoded.constData(), 10240);
#endif
    struct archive_entry *entry = nullptr;
    QByteArray bytes;
    qint64 kept = 0;
    while (status >= ARCHIVE_WARN && !cancel->load()) {
        status = archive_read_next_header(reader, &entry);
        if (status == ARCHIVE_EOF) break;
        if (status < ARCHIVE_WARN) break;
        const char *raw = archive_entry_pathname(entry);
        QString name = raw ? decodeName(QByteArray(raw)) : QString();
        name = safeName(name);
        if (name != link) {
            status = archive_read_data_skip(reader);
            continue;
        }
        for (;;) {
            const void *chunk = nullptr;
            size_t size = 0;
            la_int64_t offset = 0;
            const int chunkStatus = archive_read_data_block(reader, &chunk, &size, &offset);
            if (chunkStatus == ARCHIVE_EOF) break;
            if (chunkStatus < ARCHIVE_WARN) {
                error = QString::fromUtf8(archive_error_string(reader));
                return {};
            }
            kept += size;
            if (kept > maxNestedMemberBytes) {
                error = QStringLiteral("Nested archive exceeds Kog's 512 MiB safety limit");
                return {};
            }
            bytes.append(static_cast<const char *>(chunk), static_cast<qsizetype>(size));
        }
        return bytes;
    }
    if (status < ARCHIVE_WARN && error.isEmpty())
        error = QString::fromUtf8(archive_error_string(reader));
    if (error.isEmpty()) error = QStringLiteral("Archive entry not found: %1").arg(link);
    return {};
}
} // namespace

KogArchiveListing kogListArchiveLocation(const QString &archive, const QString &entry,
    const std::shared_ptr<std::atomic_bool> &cancel)
{
    KogArchiveListing outer = kogListArchive(archive, cancel);
    if (!outer.error.isEmpty() || cancel->load()) return outer;
    QStringList rest = entry.split('/', Qt::SkipEmptyParts);
    if (rest.isEmpty()) return outer;
    QString diskFile = archive;
    KogArchiveListing current = outer;
    QStringList consumed;
    std::vector<std::unique_ptr<QTemporaryFile>> temps;
    for (int depth = 0; ; ++depth) {
        if (rest.isEmpty()) {
            if (consumed.isEmpty()) return current;
            KogArchiveListing scoped;
            scoped.error = current.error;
            const QString prefix = consumed.join('/') + '/';
            for (auto it = current.entries.cbegin(); it != current.entries.cend(); ++it)
                scoped.entries.insert(prefix + it.key(), it.value());
            return scoped;
        }
        QString link;
        int linkLength = 0;
        for (int length = rest.size(); length >= 1; --length) {
            const QString candidate = rest.mid(0, length).join('/');
            const auto found = current.entries.constFind(candidate);
            if (found == current.entries.cend() || found.value()) continue;
            if (!kogIsArchiveName(candidate)) continue;
            link = candidate;
            linkLength = length;
            break;
        }
        if (link.isEmpty()) {
            if (consumed.isEmpty()) return current;
            KogArchiveListing missing;
            missing.error = QStringLiteral("Archive entry not found: %1").arg(entry);
            return missing;
        }
        if (depth == maxNestedDepth) {
            KogArchiveListing denied;
            denied.error = QStringLiteral("archive nesting exceeds Kog's 4-level safety limit");
            return denied;
        }
        QString error;
        std::unique_ptr<struct archive, decltype(&archive_read_free)> reader(
            archive_read_new(), archive_read_free);
        const QByteArray bytes = readNestedMember(reader.get(), diskFile, link, error, cancel);
        if (!error.isEmpty() || cancel->load()) {
            KogArchiveListing failed;
            failed.error = error;
            return failed;
        }
        auto temp = std::make_unique<QTemporaryFile>(
            QDir::tempPath() + "/kog-nested-XXXXXX." + QFileInfo(link).suffix());
        if (!temp->open()) {
            KogArchiveListing failed;
            failed.error = QStringLiteral("Could not stage a nested archive for browsing");
            return failed;
        }
        temp->write(bytes);
        temp->flush();
        diskFile = temp->fileName();
        temps.push_back(std::move(temp));
        KogArchiveListing level;
        {
            std::unique_ptr<struct archive, decltype(&archive_read_free)> levelReader(
                archive_read_new(), archive_read_free);
            const QFileInfo tempInfo(diskFile);
            openListingReader(levelReader.get(), tempInfo);
#ifdef Q_OS_WIN
            const auto native = diskFile.toStdWString();
            int status = archive_read_open_filename_w(levelReader.get(), native.c_str(), 10240);
#else
            const auto encoded = QFile::encodeName(diskFile);
            int status = archive_read_open_filename(levelReader.get(), encoded.constData(), 10240);
#endif
            status = readArchiveEntries(levelReader.get(), status, tempInfo, level, cancel);
            if (status < ARCHIVE_WARN && level.error.isEmpty())
                level.error = QString::fromUtf8(archive_error_string(levelReader.get()));
            if (!level.error.isEmpty()) level.entries.clear();
        }
        if (!level.error.isEmpty() || cancel->load()) return level;
        consumed << link;
        rest = rest.mid(linkLength);
        current = level;
    }
}
