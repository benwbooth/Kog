#include "kog_file_tree_search.h"
#include <QtCore/QCoreApplication>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QTemporaryDir>
#include <QtCore/QThread>
#include <QtQml/QQmlComponent>
#include <QtQml/QQmlContext>
#include <QtQml/QQmlEngine>
#include <QtWidgets/QApplication>
#include <cstdio>
#include <cstdlib>
#include <functional>

static void check(bool value, const char *message)
{
    if (!value) { std::fprintf(stderr, "%s\n", message); std::exit(1); }
}

static void settle(KogFileTreeSearch &model)
{
    QElapsedTimer timer;
    timer.start();
    while (model.searching() && timer.elapsed() < 10000) {
        QCoreApplication::processEvents();
        QThread::msleep(2);
    }
    check(!model.searching(), "Search failed to complete");
}

static void waitFor(const std::function<bool()> &ready, const char *message)
{
    QElapsedTimer timer;
    timer.start();
    while (!ready() && timer.elapsed() < 10000) {
        QCoreApplication::processEvents();
        QThread::msleep(2);
    }
    check(ready(), message);
}

static QModelIndex childNamed(KogFileTreeSearch &model, const QModelIndex &parent,
                              const QString &name)
{
    for (int row = 0; row < model.rowCount(parent); ++row) {
        auto child = model.index(row, 0, parent);
        if (child.data(QFileSystemModel::FileNameRole).toString() == name) return child;
    }
    return {};
}

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    QTemporaryDir fixture;
    check(fixture.isValid(), "Temporary fixture directory");
    QDir base(fixture.path());
    check(base.mkpath("Album/Disc 1"), "Create nested unopened folders");
    check(base.mkpath("Other"), "Create other folder");
    check(base.mkpath("Album/Empty"), "Create empty folder");
    for (const auto &name : {"Album/Disc 1/日本語 Theme.mid", "Other/unrelated.mp3", "Root Theme.flac"}) {
        QFile file(base.filePath(QString::fromUtf8(name)));
        check(file.open(QIODevice::WriteOnly), "Create fixture file");
    }
    KogFileTreeSearch model;
    model.setRootPath(base.absolutePath());
    QQmlEngine engine;
    engine.rootContext()->setContextProperty("testModel", &model);
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import QtQuick.Controls
        Window {
            width: 360; height: 500; visible: true
            TreeView {
                id: tree
                objectName: "tree"
                anchors.fill: parent
                model: testModel
                rootIndex: testModel.viewRootIndex
                delegate: TreeViewDelegate {
                    required property string fileName
                    required property string filePath
                    objectName: filePath
                    text: fileName
                }
            }
            Connections {
                target: testModel
                function onSearchResultsChanged() {
                    if (testModel.searchText.length && !testModel.searching)
                        Qt.callLater(function() {
                            tree.forceLayout()
                            if (!testModel.searchText.length || testModel.searching) return
                            for (let row = 0; row < tree.rows; ++row) {
                                if (testModel.isSearchAncestor(tree.index(row, 0))) {
                                    tree.expand(row)
                                    tree.forceLayout()
                                }
                            }
                        })
                }
            }
        }
    )", QUrl("file:///kog-file-tree-search-test.qml"));
    std::unique_ptr<QObject> view(component.create());
    if (!view) std::fprintf(stderr, "%s\n", qPrintable(component.errorString()));
    check(bool(view), "Create real QML TreeView with the search model");
    model.setSearchText("THEME");
    check(model.searching(), "Search must run asynchronously");
    settle(model);
    check(model.roleNames().value(QFileSystemModel::FileNameRole) == "fileName",
          "Search snapshots retain filesystem role names for QML");
    auto root = model.viewRootIndex();
    check(model.rowCount(root) == 2, "Only ancestors and matching top-level file are visible");
    auto album = model.index(0, 0, root);
    auto disc = model.index(0, 0, album);
    auto song = model.index(0, 0, disc);
    check(model.isDir(album) && model.isDir(disc) && !model.isDir(song), "Directory roles");
    check(model.filePath(song) == base.filePath(QString::fromUtf8("Album/Disc 1/日本語 Theme.mid")),
          "Search index maps to the real path for activation and drag-and-drop");
    check(song.data(QFileSystemModel::FileNameRole).toString() == QString::fromUtf8("日本語 Theme.mid"),
          "File name role for QML and MIME icons");
    auto *tree = view->findChild<QObject *>("tree");
    QElapsedTimer layoutTimer;
    layoutTimer.start();
    while (tree->property("rows").toInt() != 4 && layoutTimer.elapsed() < 5000) {
        QCoreApplication::processEvents();
        QThread::msleep(5);
    }
    if (tree->property("rows").toInt() != 4)
        std::fprintf(stderr, "TreeView rows=%d; root valid=%d; model children=%d\n",
                     tree->property("rows").toInt(),
                     tree->property("rootIndex").value<QModelIndex>().isValid(),
                     model.rowCount(model.viewRootIndex()));
    check(tree->property("rows").toInt() == 4, "QML auto-expands matching ancestors");
    model.setSearchText(QString::fromUtf8("日本語 theme"));
    settle(model);
    check(model.rowCount(model.viewRootIndex()) == 1, "Unicode and multiple search terms");

    model.setSearchText("Album");
    settle(model);
    waitFor([&] { return tree->property("rows").toInt() == 1; },
            "A matching folder starts collapsed, without enumerating its descendants");
    QPersistentModelIndex matchedAlbum(model.index(0, 0, model.viewRootIndex()));
    check(model.hasChildren(matchedAlbum) && model.canFetchMore(matchedAlbum),
          "Folder-name match must advertise expandable, lazy contents");
    check(QMetaObject::invokeMethod(tree, "toggleExpanded", Q_ARG(int, 0)),
          "Use the same single-click expansion as the production tree");
    waitFor([&] { return tree->property("rows").toInt() == 3; },
            "Expanding a search result loads nonmatching child folders");
    QPersistentModelIndex matchedDisc(childNamed(model, matchedAlbum, "Disc 1"));
    check(matchedDisc.isValid() && model.hasChildren(matchedDisc), "Nested folders are also expandable");
    check(QMetaObject::invokeMethod(tree, "toggleExpanded", Q_ARG(int, 1)), "Expand nested folder");
    waitFor([&] { return tree->property("rows").toInt() == 4; },
            "Nested expansion reveals nonmatching songs in the real QML view");
    auto browsedSong = model.index(0, 0, matchedDisc);
    check(model.filePath(browsedSong) == base.filePath(QString::fromUtf8("Album/Disc 1/日本語 Theme.mid"))
              && !model.isDir(browsedSong),
          "Lazy results retain actual paths and directory roles for drag/drop and MIME icons");
    check(QMetaObject::invokeMethod(tree, "toggleExpanded", Q_ARG(int, 0)), "Collapse folder");
    check(QMetaObject::invokeMethod(tree, "toggleExpanded", Q_ARG(int, 0)), "Reopen folder");
    check(model.rowCount(matchedAlbum) == 2 && !model.canFetchMore(matchedAlbum),
          "Reopening a folder does not duplicate its children");
    auto empty = childNamed(model, matchedAlbum, "Empty");
    model.fetchMore(empty);
    waitFor([&] { return !model.hasChildren(empty); }, "Empty folders finish loading without phantom children");

    // Both the directory and an existing child match: merging lazy contents
    // must retain the original child index and avoid duplicate rows.
    check(base.mkpath("Album/Album bonus"), "Create nested matching folder");
    model.setSearchText("album ");
    settle(model);
    QPersistentModelIndex mergeAlbum(model.index(0, 0, model.viewRootIndex()));
    QPersistentModelIndex bonus(childNamed(model, mergeAlbum, "Album bonus"));
    check(bonus.isValid(), "Initial snapshot includes the matching descendant");
    model.fetchMore(mergeAlbum);
    waitFor([&] { return model.rowCount(mergeAlbum) == 3; }, "Merge existing matches and unfiltered contents");
    check(bonus.isValid() && model.filePath(bonus) == base.filePath("Album/Album bonus"),
          "Existing result indexes survive lazy insertion and sorting");
    model.fetchMore(bonus);
    model.setSearchText("missing");
    settle(model);
    QCoreApplication::processEvents();
    check(model.rowCount(model.viewRootIndex()) == 0, "Cancelled folder load cannot leak into a new search");
    model.setSearchText("unrelated");
    model.setSearchText("missing");
    settle(model);
    check(model.rowCount(model.viewRootIndex()) == 0, "Superseded search cannot replace current results");
    model.setSearchText("theme");
    model.setRootPath(base.filePath("Other"));
    settle(model);
    check(model.rowCount(model.viewRootIndex()) == 0, "Changing root cancels old search");
    model.setSearchText("");
    check(!model.searching() && model.searchStatus().isEmpty(), "Clear cancels search and restores browsing");
    check(qobject_cast<QFileSystemModel *>(model.sourceModel()), "Live filesystem restored");
    for (int i = 0; i < 2010; ++i) {
        QFile file(base.filePath(QString("Other/limit-%1.mid").arg(i)));
        check(file.open(QIODevice::WriteOnly), "Create limit fixture");
    }
    model.setSearchText("limit-");
    settle(model);
    check(model.rowCount(model.viewRootIndex()) == 2000, "Search result bound");
    check(model.searchStatus().contains("narrow"), "Result limit is explained");
    model.setRootPath(base.absolutePath());
    model.setSearchText("Other");
    settle(model);
    QPersistentModelIndex other(model.index(0, 0, model.viewRootIndex()));
    model.fetchMore(other);
    waitFor([&] { return model.rowCount(other) == 2011; },
            "Explicit expansion browses all children in batches, independently of search match limits");
    model.setSearchText("Album");
    settle(model);
    model.fetchMore(model.index(0, 0, model.viewRootIndex()));
    model.setSearchText("");
    QCoreApplication::processEvents();
    check(qobject_cast<QFileSystemModel *>(model.sourceModel()), "Clearing search cancels pending folder expansion");
    model.setSearchText("limit"); // Destruction during a scan is safe.
    std::puts("File tree search tests passed");
}
