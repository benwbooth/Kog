#include "kog_file_tree_search.h"

#include <QtConcurrent/QtConcurrentRun>
#include <QtCore/QDirIterator>
#include <QtCore/QFutureWatcher>
#include <QtCore/QRegularExpression>
#include <algorithm>

namespace {
constexpr int directoryRole = Qt::UserRole + 100;
constexpr int matchLimit = 2000;
constexpr int nodeLimit = 12000;
struct SearchResult {
    QMap<QString, bool> paths;
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

KogFileTreeSearch::KogFileTreeSearch(QObject *parent)
    : QSortFilterProxyModel(parent)
{
    m_results.setItemRoleNames(m_files.roleNames());
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
        ? m_files.index(m_root) : m_results.index(0, 0));
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
        m_searching = false;
        m_status.clear();
        emit viewRootIndexChanged();
        emit searchStateChanged();
        emit searchResultsChanged();
        return;
    }
    // Do not leave old-query paths selectable while the next scan is running.
    m_results.clear();
    auto *root = new QStandardItem(QFileInfo(m_root).fileName());
    root->setData(m_root, QFileSystemModel::FilePathRole);
    m_results.appendRow(root);
    setSourceModel(&m_results);
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
            item->setEditable(false);
            parents.value(relative.path())->appendRow(item);
            parents.insert(it.key(), item);
        }
        // Attach the completed tree once, rather than dispatching thousands of
        // incremental proxy/QML layout updates during a large result set.
        m_results.clear();
        m_results.appendRow(resultRoot);
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
