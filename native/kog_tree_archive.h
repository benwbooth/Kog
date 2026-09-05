#pragma once

#include <QtCore/QMap>
#include <QtCore/QString>
#include <atomic>
#include <functional>
#include <memory>

struct KogArchiveLocation {
    QString archive;
    QString entry;
    bool directory = false;
};
struct KogArchiveListing {
    QMap<QString, bool> entries;
    QString error;
    bool fromCache = false;
};
#ifdef KOG_TREE_TESTS
void kogClearArchiveMemoryCache();
void kogSetArchiveReadTestHook(std::function<void()> hook);
#endif

bool kogIsArchive(const QString &path);
QString kogArchiveUrl(const QString &archive, const QString &entry, bool directory);
KogArchiveLocation kogArchiveLocation(const QString &url);
KogArchiveListing kogListArchive(const QString &path,
    const std::shared_ptr<std::atomic_bool> &cancel);
// Installed once before any tree workers start, to share Rust's name decoder.
void kogSetArchiveNameDecoder(std::function<QString(const QByteArray &)> decoder);
