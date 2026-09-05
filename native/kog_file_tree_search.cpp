#include "kog_file_tree_search.h"
#include "kog_tree_archive.h"

#include <QtConcurrent/QtConcurrentRun>
#include <QtCore/QDirIterator>
#include <QtCore/QFutureWatcher>
#include <QtCore/QRegularExpression>
#include <QtCore/QSet>
#include <QtCore/QTimer>
#include <QtCore/QFileSystemWatcher>
#include <QtCore/QElapsedTimer>
#include <QtCore/QMutex>
#include <QtCore/QMimeDatabase>
#include <QtCore/QTextBoundaryFinder>
#include <optional>
#include <algorithm>

namespace {
constexpr int directoryRole = Qt::UserRole + 100;
constexpr int browseRole = directoryRole + 1;
constexpr int fetchStateRole = directoryRole + 2;
constexpr int containerRole = directoryRole + 3;
constexpr int iconRole = directoryRole + 4;
constexpr int matchLimit = 2000;
constexpr int nodeLimit = 12000;
struct TreeEntry {
    QString path;
    bool directory = false;
    bool container = false;
    QString icon;

    TreeEntry() = default;
    TreeEntry(QString location, bool isDirectory, bool isContainer)
        : path(std::move(location)), directory(isDirectory), container(isContainer)
    {
        // All entries are constructed by workers. Never stat a music file or
        // load the MIME database from a QML delegate on the input thread.
        if (directory) { icon = QStringLiteral("folder"); return; }
        const auto member = kogArchiveLocation(path);
        const auto name = member.archive.isEmpty() ? path : member.entry;
        thread_local QMimeDatabase database;
        thread_local QHash<QString, QString> icons;
        const auto suffix = QFileInfo(name).suffix().toLower();
        auto found = icons.constFind(suffix);
        if (found == icons.cend()) {
            auto mime = database.mimeTypeForFile(name, QMimeDatabase::MatchExtension);
            auto value = mime.iconName();
            if (value.isEmpty()) value = QStringLiteral("audio-x-generic");
            found = icons.insert(suffix, value);
        }
        icon = *found;
    }
};
struct SearchResult {
    QMap<QString, TreeEntry> paths;
    QSet<QString> matchingDirectories;
    int matches = 0;
    bool limited = false;
    int unreadableArchives = 0;
    int filesScanned = 0;
    int archivesScanned = 0;
    int archiveCount = 0;
    bool scanningArchives = false;
};
struct SearchProgress {
    QMutex mutex;
    std::optional<SearchResult> latest;
};

SearchResult scan(const QString &root, const QString &query,
                  const std::shared_ptr<std::atomic_bool> &cancel,
                  const std::shared_ptr<SearchProgress> &progress)
{
    SearchResult result;
    QElapsedTimer throttle;
    throttle.start();
    auto publish = [&](bool force = false) {
        if (!force && throttle.elapsed() < 100) return;
        QMutexLocker lock(&progress->mutex);
        progress->latest = result; // One coalesced snapshot, never an unbounded GUI event queue.
        throttle.restart();
    };
    const auto words = query.normalized(QString::NormalizationForm_KC).toCaseFolded().split(
        QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
    const QDir base(root);
    auto matches = [&](const QString &name) {
        const auto folded = name.normalized(QString::NormalizationForm_KC).toCaseFolded();
        return std::all_of(words.begin(), words.end(), [&](const auto &word) { return folded.contains(word); });
    };
    auto match = [&](const QString &relative, const TreeEntry &entry,
                     const QString &archiveRelative = QString()) {
        result.paths.insert(relative, entry);
        if (entry.container) result.matchingDirectories.insert(relative);
        auto parent = QFileInfo(relative).path();
        while (parent != "." && !parent.isEmpty()) {
            if (!archiveRelative.isEmpty() && parent.startsWith(archiveRelative + '/')) {
                result.paths.insert(parent, {kogArchiveUrl(base.filePath(archiveRelative),
                    parent.mid(archiveRelative.size() + 1), true), true, true});
            } else {
                const auto path = base.filePath(parent);
                const bool directory = parent != archiveRelative;
                result.paths.insert(parent, {path, directory, true});
            }
            parent = QFileInfo(parent).path();
        }
        ++result.matches;
        result.limited = result.matches >= matchLimit || result.paths.size() >= nodeLimit;
    };
    QDirIterator entries(root, QDir::AllEntries | QDir::NoDotAndDotDot,
                         QDirIterator::Subdirectories); // Never follow directory symlinks.
    QStringList archives;
    while (!cancel->load(std::memory_order_relaxed) && entries.hasNext()) {
        entries.next();
        const auto info = entries.fileInfo();
        const auto relative = base.relativeFilePath(info.absoluteFilePath());
        const bool archive = info.isFile() && kogIsArchive(info.absoluteFilePath());
        if (matches(info.fileName()))
            match(relative, {info.absoluteFilePath(), info.isDir(), info.isDir() || archive});
        if (result.limited) break;
        if (archive) archives.append(info.absoluteFilePath());
        ++result.filesScanned;
        publish();
    }
    // Ordinary files must not wait behind a slow solid archive. Search the
    // filesystem first, then stream archive matches into the same stable tree.
    result.archiveCount = archives.size();
    result.scanningArchives = true;
    publish(true);
    for (const auto &path : archives) {
        if (cancel->load() || result.limited) break;
        const auto relative = base.relativeFilePath(path);
        const auto listing = kogListArchive(path, cancel);
        if (!listing.error.isEmpty()) ++result.unreadableArchives;
        for (auto it = listing.entries.cbegin(); it != listing.entries.cend() && !cancel->load(); ++it) {
            // Comparing names is cheap. Only construct encoded member URLs
            // and ancestor paths for actual matches, not every indexed entry.
            if (matches(QFileInfo(it.key()).fileName()))
                match(relative + '/' + it.key(),
                    {kogArchiveUrl(path, it.key(), it.value()), it.value(), it.value()}, relative);
            if (result.limited) break;
            publish();
        }
        ++result.archivesScanned;
        publish();
    }
    return result;
}
}

// Search ancestors stay filtered. A matching folder, and directories opened
// inside it, can lazily expose their real contents without a recursive rescan.
class KogSearchResults : public QStandardItemModel {
public:
    KogSearchResults() : m_cancel(std::make_shared<std::atomic_bool>(false))
    {
        auto roles = roleNames();
        roles.insert(QFileSystemModel::FileNameRole, "fileName");
        roles.insert(QFileSystemModel::FilePathRole, "filePath");
        roles.insert(iconRole, "fileIcon");
        setItemRoleNames(roles);
        auto changed = [this](const QString &path) {
            const auto target = watched.value(path);
            if (target.isValid()) load(target, true);
        };
        connect(&watcher, &QFileSystemWatcher::directoryChanged, this, changed);
        connect(&watcher, &QFileSystemWatcher::fileChanged, this, [this, changed](const QString &path) {
            const auto target = watched.value(path);
            if (target.isValid()) removeRows(0, rowCount(target), target);
            changed(path);
        });
    }
    ~KogSearchResults() override { m_cancel->store(true); }
    std::function<void(const QString &)> reportError;

    void resetResults(QStandardItem *root)
    {
        m_cancel->store(true);
        m_cancel = std::make_shared<std::atomic_bool>(false);
        const auto watchPaths = watcher.directories() + watcher.files();
        if (!watchPaths.isEmpty()) watcher.removePaths(watchPaths);
        watched.clear();
        clear();
        if (root) appendRow(root);
    }

    bool hasChildren(const QModelIndex &parent = {}) const override
    {
        return QStandardItemModel::hasChildren(parent) || canFetchMore(parent)
            || parent.data(fetchStateRole).toInt() == 1;
    }

    bool canFetchMore(const QModelIndex &parent) const override
    {
        return parent.isValid() && parent.data(browseRole).toBool()
            && parent.data(fetchStateRole).toInt() == 0;
    }

    void fetchMore(const QModelIndex &parent) override
    {
        if (!canFetchMore(parent)) return;
        load(parent, false);
    }

private:
    struct Entries {
        QMap<QString, TreeEntry> rows;
        QString error;
    };
    std::shared_ptr<std::atomic_bool> m_cancel;
    QFileSystemWatcher watcher;
    QHash<QString, QPersistentModelIndex> watched;

    void load(const QModelIndex &parent, bool refresh)
    {
        if (parent.data(fetchStateRole).toInt() == 1) return;
        setData(parent, 1, fetchStateRole);
        const QPersistentModelIndex target(parent);
        const auto cancel = m_cancel;
        const auto path = parent.data(QFileSystemModel::FilePathRole).toString();
        auto *job = new QFutureWatcher<Entries>(this);
        connect(job, &QFutureWatcher<Entries>::finished, this,
                [this, job, target, cancel, path, refresh] {
            job->deleteLater();
            if (cancel->load() || !target.isValid()) return;
            auto entries = std::make_shared<Entries>(job->result());
            if (!entries->error.isEmpty()) {
                setData(target, 2, fetchStateRole);
                if (reportError) reportError(QFileInfo(path).fileName() + ": " + entries->error);
                return;
            }
            for (int row = rowCount(target) - 1; row >= 0; --row) {
                const auto name = index(row, 0, target).data(QFileSystemModel::FileNameRole).toString();
                if (refresh && !entries->rows.contains(name)) removeRow(row, target);
                else entries->rows.remove(name);
            }
            if (!path.startsWith("kog-archive:")) {
                watcher.addPath(path);
                watched.insert(path, target);
            }
            appendBatch(target, entries, cancel);
        });
        job->setFuture(QtConcurrent::run([path, cancel] {
            Entries entries;
            auto location = kogArchiveLocation(path);
            if (kogIsArchive(path)) location = {path, {}, true};
            if (!location.archive.isEmpty()) {
                const auto listing = kogListArchive(location.archive, cancel);
                entries.error = listing.error;
                const auto prefix = location.entry.isEmpty() ? QString() : location.entry + '/';
                for (auto it = listing.entries.cbegin(); it != listing.entries.cend(); ++it) {
                    if (!it.key().startsWith(prefix)) continue;
                    const auto name = it.key().mid(prefix.size());
                    if (name.isEmpty() || name.contains('/')) continue;
                    entries.rows.insert(name, {kogArchiveUrl(location.archive, it.key(), it.value()),
                                               it.value(), it.value()});
                }
                return entries;
            }
            QDirIterator dir(path, QDir::AllEntries | QDir::NoDotAndDotDot);
            while (!cancel->load() && dir.hasNext()) {
                dir.next();
                const auto info = dir.fileInfo();
                entries.rows.insert(info.fileName(), {info.absoluteFilePath(), info.isDir(),
                    info.isDir() || kogIsArchive(info.absoluteFilePath())});
            }
            return entries;
        }));
    }

    void appendBatch(const QPersistentModelIndex &target,
                     const std::shared_ptr<Entries> &entries,
                     const std::shared_ptr<std::atomic_bool> &cancel)
    {
        if (cancel->load() || !target.isValid()) return;
        auto *parent = itemFromIndex(target);
        QList<QStandardItem *> batch;
        for (int count = 0; count < 256 && !entries->rows.isEmpty(); ++count) {
            auto first = entries->rows.begin();
            const auto name = first.key();
            const auto entry = first.value();
            entries->rows.erase(first);
            auto *item = new QStandardItem(name);
            item->setData(name, QFileSystemModel::FileNameRole);
            item->setData(entry.path, QFileSystemModel::FilePathRole);
            item->setData(entry.icon, iconRole);
            item->setData(entry.directory, directoryRole);
            item->setData(entry.container, containerRole);
            item->setData(entry.container, browseRole);
            item->setEditable(false);
            batch.append(item);
        }
        parent->appendRows(batch);
        if (!entries->rows.isEmpty()) {
            // Yield between batches so a large folder does not monopolize the UI.
            QTimer::singleShot(0, this, [this, target, entries, cancel] {
                appendBatch(target, entries, cancel);
            });
        } else {
            parent->sortChildren(0);
            setData(target, 2, fetchStateRole);
        }
    }
};

KogFileTreeSearch::KogFileTreeSearch(QObject *parent)
    : QSortFilterProxyModel(parent), m_files(std::make_unique<KogSearchResults>()),
      m_results(std::make_unique<KogSearchResults>())
{
    auto error = [this](const QString &message) { m_status = message; emit searchStateChanged(); };
    m_files->reportError = error;
    m_results->reportError = error;
    setSourceModel(m_files.get());
    setDynamicSortFilter(false);
}

KogFileTreeSearch::~KogFileTreeSearch()
{
    if (m_cancel) m_cancel->store(true, std::memory_order_relaxed);
}

QModelIndex KogFileTreeSearch::setRootPath(const QString &path)
{
    m_root = QDir::cleanPath(path);
    auto *root = new QStandardItem(QFileInfo(m_root).fileName());
    root->setData(m_root, QFileSystemModel::FilePathRole);
    root->setData(QFileInfo(m_root).fileName(), QFileSystemModel::FileNameRole);
    root->setData(QStringLiteral("folder"), iconRole);
    root->setData(true, directoryRole);
    root->setData(true, containerRole);
    root->setData(true, browseRole);
    m_files->resetResults(root);
    m_files->fetchMore(m_files->index(0, 0));
    startSearch();
    return viewRootIndex();
}

QModelIndex KogFileTreeSearch::viewRootIndex() const
{
    return mapFromSource(sourceModel()->index(0, 0));
}

bool KogFileTreeSearch::isSearchAncestor(const QModelIndex &index) const
{
    return sourceModel() == m_results.get() && index.data(containerRole).toBool()
        && !index.data(browseRole).toBool();
}

QString KogFileTreeSearch::filePath(const QModelIndex &index) const
{
    return index.data(QFileSystemModel::FilePathRole).toString();
}

QString KogFileTreeSearch::displayPath(const QString &path) const
{
    const auto location = kogArchiveLocation(path);
    return location.archive.isEmpty() ? path : location.archive + " :: " + location.entry;
}

QString KogFileTreeSearch::highlightedName(const QString &name, const QString &query,
                                         const QString &elidedName, bool wholeQuery) const
{
    const auto fold = [wholeQuery](const QString &text) {
        return wholeQuery ? text.toLower()
            : text.normalized(QString::NormalizationForm_KC).toCaseFolded();
    };
    // Match the scanner's Unicode normalization, but retain offsets into the
    // original spelling. Grapheme boundaries keep accents, kana and emoji intact
    // when normalization changes the length of a character sequence.
    QString folded;
    QList<QPair<int, int>> original;
    QTextBoundaryFinder graphemes(QTextBoundaryFinder::Grapheme, name);
    int start = 0;
    for (int end = graphemes.toNextBoundary(); end >= 0; end = graphemes.toNextBoundary()) {
        const auto part = fold(name.mid(start, end - start));
        folded += part;
        for (qsizetype i = 0; i < part.size(); ++i) original.append({start, end});
        start = end;
    }
    QList<bool> marked(name.size(), false);
    const auto words = wholeQuery ? QStringList{fold(query).trimmed()}
        : fold(query).split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
    for (const auto &word : words) {
        if (word.isEmpty()) continue;
        qsizetype from = 0;
        while ((from = folded.indexOf(word, from)) >= 0) {
            const int begin = original[from].first;
            const int end = original[from + word.size() - 1].second;
            for (int i = begin; i < end; ++i) marked[i] = true;
            ++from; // Include repeated and overlapping occurrences.
        }
    }
    // Elide plain text first: Qt's rich-text renderer does not provide elision.
    // Retain highlighting on the visible part of a truncated match, but never
    // on the ellipsis. Only visible delegates request this, not the entire tree.
    qsizetype visible = 0;
    while (visible < name.size() && visible < elidedName.size()
           && name[visible] == elidedName[visible]) ++visible;
    QString html = QStringLiteral("<span style=\"white-space: pre;\">");
    for (qsizetype at = 0; at < visible;) {
        const bool highlight = marked[at];
        qsizetype end = at + 1;
        while (end < visible && marked[end] == highlight) ++end;
        if (highlight)
            html += QStringLiteral("<span style=\"background-color: #f6d65a; color: #1b1b1b;\">");
        html += name.mid(at, end - at).toHtmlEscaped();
        if (highlight) html += QStringLiteral("</span>");
        at = end;
    }
    html += elidedName.mid(visible).toHtmlEscaped();
    return html + QStringLiteral("</span>");
}

bool KogFileTreeSearch::isDir(const QModelIndex &index) const
{
    return index.data(containerRole).toBool();
}

void KogFileTreeSearch::setSearchText(const QString &query)
{
    if (m_query == query) return;
    m_query = query;
    emit searchTextChanged();
    startSearch();
}

void KogFileTreeSearch::startSearch()
{
    if (m_cancel) m_cancel->store(true, std::memory_order_relaxed);
    const auto generation = ++m_generation;
    if (m_query.trimmed().isEmpty() || m_root.isEmpty()) {
        setSourceModel(m_files.get());
        m_results->resetResults(nullptr);
        m_searching = false;
        m_status.clear();
        emit viewRootIndexChanged();
        emit searchStateChanged();
        emit searchResultsChanged();
        return;
    }
    // Do not leave old-query paths selectable while the next scan is running.
    auto *root = new QStandardItem(QFileInfo(m_root).fileName());
    root->setData(m_root, QFileSystemModel::FilePathRole);
    root->setData(QFileInfo(m_root).fileName(), QFileSystemModel::FileNameRole);
    root->setData(QStringLiteral("folder"), iconRole);
    m_results->resetResults(root);
    setSourceModel(m_results.get());
    m_searching = true;
    m_status = tr("Searching subfolders…");
    emit viewRootIndexChanged();
    emit searchStateChanged();
    emit searchResultsChanged();
    m_cancel = std::make_shared<std::atomic_bool>(false);
    const auto progress = std::make_shared<SearchProgress>();
    struct Merge {
        std::optional<SearchResult> current;
        std::optional<SearchResult> pending;
        QMap<QString, TreeEntry>::const_iterator next;
        QHash<QString, QPersistentModelIndex> known;
        bool finalPending = false;
        bool finalCurrent = false;
        bool changed = false;
        QElapsedTimer notification;
    };
    const auto merge = std::make_shared<Merge>();
    merge->notification.start();
    auto *watcher = new QFutureWatcher<SearchResult>(this);
    auto *updates = new QTimer(watcher);
    auto *batches = new QTimer(watcher);
    batches->setInterval(4);
    connect(batches, &QTimer::timeout, this, [this, generation, merge, watcher, batches] {
        if (generation != m_generation) { watcher->deleteLater(); return; }
        if (!merge->current) {
            if (!merge->pending) { batches->stop(); return; }
            merge->current = std::move(merge->pending);
            merge->pending.reset();
            merge->next = merge->current->paths.cbegin();
            merge->finalCurrent = merge->finalPending;
        }
        const auto &result = *merge->current;
        QElapsedTimer budget;
        budget.start();
        QHash<QString, QStandardItem *> parents;
        parents.insert(QStringLiteral("."), m_results->item(0));
        QHash<QStandardItem *, QList<QStandardItem *>> insertions;
        // QMap's path ordering puts each parent before its descendants.
        // Keep each GUI transaction small; even cached searches can have
        // thousands of nodes, and input must run between insertion batches.
        int count = 0;
        for (auto &it = merge->next; it != result.paths.cend() && count < 128
             && budget.elapsed() < 3; ++it, ++count) {
            const QFileInfo relative(it.key());
            auto *parent = parents.value(relative.path());
            if (!parent) parent = m_results->itemFromIndex(merge->known.value(relative.path()));
            if (!parent) continue; // A filesystem refresh may have removed an ancestor.
            auto *item = m_results->itemFromIndex(merge->known.value(it.key()));
            // A user may have already expanded a matching folder while the
            // search was running. Merge with its lazy children, never duplicate.
            if (!item && parent->data(fetchStateRole).toInt() != 0) {
                for (int row = 0; row < parent->rowCount(); ++row) {
                    auto *child = parent->child(row);
                    if (child->data(QFileSystemModel::FilePathRole).toString() == it->path) {
                        item = child;
                        break;
                    }
                }
            }
            if (!item) {
                item = new QStandardItem(relative.fileName());
                item->setData(relative.fileName(), QFileSystemModel::FileNameRole);
                item->setData(it->path, QFileSystemModel::FilePathRole);
                item->setData(it->icon, iconRole);
                item->setData(it->directory, directoryRole);
                item->setData(it->container, containerRole);
                item->setEditable(false);
                if (parent->model()) insertions[parent].append(item);
                else parent->appendRow(item); // Detached subtree: no per-row QML updates.
                merge->changed = true;
            }
            const bool browse = it->container && (result.matchingDirectories.contains(it.key())
                || parent->data(browseRole).toBool());
            if (item->data(browseRole).toBool() != browse) {
                item->setData(browse, browseRole);
                merge->changed = true;
            }
            parents.insert(it.key(), item);
        }
        for (auto it = insertions.cbegin(); it != insertions.cend(); ++it)
            it.key()->appendRows(it.value());
        for (auto it = parents.cbegin(); it != parents.cend(); ++it)
            merge->known.insert(it.key(), it.value()->index());
        const bool snapshotDone = merge->next == result.paths.cend();
        const bool complete = snapshotDone && merge->finalCurrent;
        const bool wasSearching = m_searching;
        const auto previousStatus = m_status;
        m_searching = !complete;
        m_status = !complete
            ? (result.scanningArchives
                ? tr("%1 matches · Archives %2 of %3").arg(result.matches).arg(result.archivesScanned).arg(result.archiveCount)
                : tr("%1 matches · Searching folders (%2 items)").arg(result.matches).arg(result.filesScanned))
            : result.matches == 0 ? tr("No matching files or folders")
            : result.limited ? tr("%1 matches — narrow your search for more").arg(result.matches)
                             : tr("%n matching file(s) or folder(s)", nullptr, result.matches);
        if (result.unreadableArchives)
            m_status += tr(" — %n archive(s) could not be searched", nullptr, result.unreadableArchives);
        if (wasSearching != m_searching || previousStatus != m_status)
            emit searchStateChanged();
        if ((merge->changed && merge->notification.elapsed() >= 16) || snapshotDone) {
            emit searchBatchChanged();
            merge->changed = false;
            merge->notification.restart();
        }
        if (snapshotDone) merge->current.reset();
        if (complete) watcher->deleteLater();
    });
    updates->setInterval(100);
    connect(updates, &QTimer::timeout, this, [this, generation, progress, merge, batches, watcher] {
        if (generation != m_generation) { watcher->deleteLater(); return; }
        { QMutexLocker lock(&progress->mutex);
          if (progress->latest) {
              merge->pending = std::move(progress->latest);
              progress->latest.reset();
          }
        }
        if (merge->pending && !batches->isActive()) batches->start();
    });
    connect(watcher, &QFutureWatcher<SearchResult>::finished, this, [watcher, updates, batches, merge] {
        updates->stop();
        merge->pending = watcher->result();
        merge->finalPending = true;
        if (!batches->isActive()) batches->start();
    });
    // The worker owns only value data and a cancellation flag, never the model.
    watcher->setFuture(QtConcurrent::run(scan, m_root, m_query, m_cancel, progress));
    updates->start();
}
