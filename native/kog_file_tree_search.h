#pragma once

#include <QtCore/QSortFilterProxyModel>
#include <QtGui/QFileSystemModel>
#include <QtGui/QStandardItemModel>
#include <atomic>
#include <memory>

class KogSearchResults;

// Lazy filesystem/archive browsing with filesystem watches. Search uses a
// bounded background snapshot, including ancestors of files inside archives.
class KogFileTreeSearch : public QSortFilterProxyModel {
    Q_OBJECT
    Q_PROPERTY(QString searchText READ searchText WRITE setSearchText NOTIFY searchTextChanged)
    Q_PROPERTY(bool searching READ searching NOTIFY searchStateChanged)
    Q_PROPERTY(QString searchStatus READ searchStatus NOTIFY searchStateChanged)
    Q_PROPERTY(QModelIndex viewRootIndex READ viewRootIndex NOTIFY viewRootIndexChanged)
public:
    explicit KogFileTreeSearch(QObject *parent = nullptr);
    ~KogFileTreeSearch() override;
    QModelIndex setRootPath(const QString &path);
    QString filePath(const QModelIndex &index) const;
    bool isDir(const QModelIndex &index) const;
    QString searchText() const { return m_query; }
    void setSearchText(const QString &query);
    bool searching() const { return m_searching; }
    QString searchStatus() const { return m_status; }
    QModelIndex viewRootIndex() const;
    Q_INVOKABLE bool isSearchAncestor(const QModelIndex &index) const;
    Q_INVOKABLE QString displayPath(const QString &path) const;
signals:
    void searchTextChanged();
    void searchStateChanged();
    void viewRootIndexChanged();
    void searchResultsChanged();
private:
    void startSearch();
    std::unique_ptr<KogSearchResults> m_files;
    std::unique_ptr<KogSearchResults> m_results;
    QString m_root;
    QString m_query;
    QString m_status;
    bool m_searching = false;
    quint64 m_generation = 0;
    std::shared_ptr<std::atomic_bool> m_cancel;
};
