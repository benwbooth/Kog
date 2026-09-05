#include "kog_file_tree_search.h"

#include <QtConcurrent/QtConcurrentRun>
#include <QtCore/QDirIterator>
#include <QtCore/QFutureWatcher>
#include <QtCore/QRegularExpression>
#include <QtCore/QSet>
#include <QtCore/QTimer>
#include <algorithm>

namespace {
constexpr int directoryRole = Qt::UserRole + 100;
constexpr int browseRole = directoryRole + 1;
constexpr int fetchStateRole = directoryRole + 2;
constexpr int matchLimit = 2000;
constexpr int nodeLimit = 12000;
struct SearchResult {
    QMap<QString, bool> paths;
    QSet<QString> matchingDirectories;
    int matches = 0;
    bool limited = false;
};

SearchResult scan(const QString &root, const QString &query,
                  const std::shared_ptr<std::atomic_bool> &cancel)
{
    SearchResult result;
    const auto words = query.normalized(QString::NormalizationForm_KC).toCaseFolded().split(
        QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
    const QDir base(root);
    QDirIterator entries(root, QDir::AllEntries | QDir::NoDotAndDotDot,
                         QDirIterator::Subdirectories); // Never follow directory symlinks.
    while (!cancel->load(std::memory_order_relaxed) && entries.hasNext()) {
        entries.next();
        const auto info = entries.fileInfo();
        const auto name = info.fileName().normalized(QString::NormalizationForm_KC).toCaseFolded();
        if (!std::all_of(words.begin(), words.end(), [&](const auto &word) {
                return name.contains(word);
            }))
            continue;
        const auto relative = base.relativeFilePath(info.absoluteFilePath());
        result.paths.insert(relative, info.isDir());
        if (info.isDir()) result.matchingDirectories.insert(relative);
        auto parent = QFileInfo(relative).path();
        while (parent != QStringLiteral(".") && !parent.isEmpty()) {
            result.paths.insert(parent, true);
            parent = QFileInfo(parent).path();
        }
        ++result.matches;
        if (result.matches >= matchLimit || result.paths.size() >= nodeLimit) {
            result.limited = true;
            break;
        }
    }
    return result;
}
}

// Search ancestors stay filtered. A matching folder, and directories opened
// inside it, can lazily expose their real contents without a recursive rescan.
class KogSearchResults : public QStandardItemModel {
public:
    KogSearchResults() : m_cancel(std::make_shared<std::atomic_bool>(false)) {}
    ~KogSearchResults() override { m_cancel->store(true); }

    void resetResults(QStandardItem *root)
    {
        m_cancel->store(true);
        m_cancel = std::make_shared<std::atomic_bool>(false);
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
        setData(parent, 1, fetchStateRole);
        const QPersistentModelIndex target(parent);
        const auto cancel = m_cancel;
        const auto path = parent.data(QFileSystemModel::FilePathRole).toString();
        QSet<QString> existing;
        for (int row = 0; row < rowCount(parent); ++row)
            existing.insert(index(row, 0, parent).data(QFileSystemModel::FileNameRole).toString());
        auto *watcher = new QFutureWatcher<Entries>(this);
        connect(watcher, &QFutureWatcher<Entries>::finished, this,
                [this, watcher, target, cancel] {
            watcher->deleteLater();
            if (cancel->load() || !target.isValid()) return;
            appendBatch(target, std::make_shared<Entries>(watcher->result()), 0, cancel);
        });
        watcher->setFuture(QtConcurrent::run([path, existing, cancel] {
            Entries entries;
            QDirIterator dir(path, QDir::AllEntries | QDir::NoDotAndDotDot);
            while (!cancel->load() && dir.hasNext()) {
                dir.next();
                const auto info = dir.fileInfo();
                if (!existing.contains(info.fileName()))
                    entries.append({info.fileName(), info.isDir()});
            }
            std::sort(entries.begin(), entries.end(), [](const auto &a, const auto &b) {
                return a.first < b.first;
            });
            return entries;
        }));
    }

private:
    using Entries = QList<QPair<QString, bool>>;
    std::shared_ptr<std::atomic_bool> m_cancel;

    void appendBatch(const QPersistentModelIndex &target,
                     const std::shared_ptr<Entries> &entries, qsizetype offset,
                     const std::shared_ptr<std::atomic_bool> &cancel)
    {
        if (cancel->load() || !target.isValid()) return;
        auto *parent = itemFromIndex(target);
        const QDir dir(target.data(QFileSystemModel::FilePathRole).toString());
        QList<QStandardItem *> batch;
        const auto end = std::min(offset + 256, entries->size());
        for (; offset < end; ++offset) {
            const auto &[name, isDir] = entries->at(offset);
            auto *item = new QStandardItem(name);
            item->setData(name, QFileSystemModel::FileNameRole);
            item->setData(dir.filePath(name), QFileSystemModel::FilePathRole);
            item->setData(isDir, directoryRole);
            item->setData(isDir, browseRole);
            item->setEditable(false);
            batch.append(item);
        }
        parent->appendRows(batch);
        if (offset < entries->size()) {
            // Yield between batches so a large folder does not monopolize the UI.
            QTimer::singleShot(0, this, [this, target, entries, offset, cancel] {
                appendBatch(target, entries, offset, cancel);
            });
        } else {
            parent->sortChildren(0);
            setData(target, 2, fetchStateRole);
        }
    }
};

KogFileTreeSearch::KogFileTreeSearch(QObject *parent)
    : QSortFilterProxyModel(parent), m_results(std::make_unique<KogSearchResults>())
{
    m_results->setItemRoleNames(m_files.roleNames());
    setSourceModel(&m_files);
    setDynamicSortFilter(false);
}

KogFileTreeSearch::~KogFileTreeSearch()
{
    if (m_cancel) m_cancel->store(true, std::memory_order_relaxed);
}

QModelIndex KogFileTreeSearch::setRootPath(const QString &path)
{
    m_root = QDir::cleanPath(path);
    m_files.setRootPath(m_root);
    startSearch();
    return viewRootIndex();
}

QModelIndex KogFileTreeSearch::viewRootIndex() const
{
    return mapFromSource(sourceModel() == &m_files
        ? m_files.index(m_root) : m_results->index(0, 0));
}

bool KogFileTreeSearch::isSearchAncestor(const QModelIndex &index) const
{
    return sourceModel() == m_results.get() && index.data(directoryRole).toBool()
        && !index.data(browseRole).toBool();
}

QString KogFileTreeSearch::filePath(const QModelIndex &index) const
{
    return index.data(QFileSystemModel::FilePathRole).toString();
}

bool KogFileTreeSearch::isDir(const QModelIndex &index) const
{
    return sourceModel() == &m_files ? m_files.isDir(mapToSource(index))
                                   : index.data(directoryRole).toBool();
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
        setSourceModel(&m_files);
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
    m_results->resetResults(root);
    setSourceModel(m_results.get());
    m_searching = true;
    m_status = tr("Searching subfolders…");
    emit viewRootIndexChanged();
    emit searchStateChanged();
    emit searchResultsChanged();
    m_cancel = std::make_shared<std::atomic_bool>(false);
    auto *watcher = new QFutureWatcher<SearchResult>(this);
    connect(watcher, &QFutureWatcher<SearchResult>::finished, this, [this, watcher, generation] {
        watcher->deleteLater();
        if (generation != m_generation) return;
        const auto result = watcher->result();
        auto *resultRoot = new QStandardItem(QFileInfo(m_root).fileName());
        resultRoot->setData(m_root, QFileSystemModel::FilePathRole);
        QHash<QString, QStandardItem *> parents;
        parents.insert(QStringLiteral("."), resultRoot);
        // QMap's path ordering puts each parent before its descendants.
        for (auto it = result.paths.cbegin(); it != result.paths.cend(); ++it) {
            const QFileInfo relative(it.key());
            auto *item = new QStandardItem(relative.fileName());
            item->setData(relative.fileName(), QFileSystemModel::FileNameRole);
            item->setData(QDir(m_root).filePath(it.key()), QFileSystemModel::FilePathRole);
            item->setData(it.value(), directoryRole);
            item->setData(it.value() && (result.matchingDirectories.contains(it.key())
                || parents.value(relative.path())->data(browseRole).toBool()), browseRole);
            item->setEditable(false);
            parents.value(relative.path())->appendRow(item);
            parents.insert(it.key(), item);
        }
        // Attach the completed tree once, rather than dispatching thousands of
        // incremental proxy/QML layout updates during a large result set.
        m_results->resetResults(resultRoot);
        m_searching = false;
        m_status = result.matches == 0 ? tr("No matching files or folders")
            : result.limited ? tr("%1 matches — narrow your search for more").arg(result.matches)
                             : tr("%n matching file(s) or folder(s)", nullptr, result.matches);
        emit viewRootIndexChanged();
        emit searchStateChanged();
        emit searchResultsChanged();
    });
    // The worker owns only value data and a cancellation flag, never the model.
    watcher->setFuture(QtConcurrent::run(scan, m_root, m_query, m_cancel));
}
