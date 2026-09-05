#include "kog_file_tree_search.h"
#include "kog_tree_archive.h"
#include <archive.h>
#include <archive_entry.h>
#include <QtCore/QCoreApplication>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QTemporaryDir>
#include <QtCore/QThread>
#include <QtCore/QTimer>
#include <QtGui/QKeyEvent>
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

static void writeArchive(const QString &path, bool sevenZip = false)
{
    auto *writer = archive_write_new();
    check((sevenZip ? archive_write_set_format_7zip(writer) : archive_write_set_format_zip(writer)) == ARCHIVE_OK,
          "Create real archive fixture with libarchive");
    const auto filename = QFile::encodeName(path);
    check(archive_write_open_filename(writer, filename.constData()) == ARCHIVE_OK, "Open archive fixture");
    for (const auto &name : {"Disc/Hidden Tune.mid", "Disc/日本語 + #%.mid", "Other/song.flac", "../escape.mid"}) {
        auto *entry = archive_entry_new();
        archive_entry_set_pathname_utf8(entry, name);
        archive_entry_set_filetype(entry, AE_IFREG);
        archive_entry_set_perm(entry, 0644);
        archive_entry_set_size(entry, 4);
        check(archive_write_header(writer, entry) >= ARCHIVE_WARN, "Write archive header");
        check(archive_write_data(writer, "test", 4) == 4, "Write archive data");
        archive_entry_free(entry);
    }
    check(archive_write_close(writer) == ARCHIVE_OK, "Close archive fixture");
    archive_write_free(writer);
}

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    if (argc == 4 && QString::fromUtf8(argv[1]) == "--benchmark") {
        KogFileTreeSearch benchmark;
        benchmark.setRootPath(QString::fromUtf8(argv[2]));
        for (int pass = 0; pass < 3; ++pass) {
            if (pass == 1) kogClearArchiveMemoryCache(); // Measure persistent-cache reuse too.
            QElapsedTimer time;
            time.start();
            qint64 first = -1;
            const auto connection = QObject::connect(&benchmark, &KogFileTreeSearch::searchBatchChanged, [&] {
                if (first < 0 && benchmark.rowCount(benchmark.viewRootIndex())) first = time.elapsed();
            });
            benchmark.setSearchText(QString::fromUtf8(argv[3]) + QString(pass, ' '));
            while (benchmark.searching() && time.elapsed() < 180000) {
                QCoreApplication::processEvents();
                QThread::msleep(2);
            }
            check(!benchmark.searching(), "Benchmark completes");
            std::printf("Search pass %d: first results=%lld ms; complete=%lld ms; %s\n", pass,
                        static_cast<long long>(first), static_cast<long long>(time.elapsed()),
                        qPrintable(benchmark.searchStatus()));
            QObject::disconnect(connection);
        }
        return 0;
    }
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
                opacity: searchLayout.ready ? 1 : 0
                reuseItems: false
                delegate: TreeViewDelegate {
                    required property string fileName
                    required property string filePath
                    required property string fileIcon
                    icon.name: fileIcon
                    objectName: filePath
                    text: fileName
                }
            }
            TreeSearchLayout {
                id: searchLayout
                objectName: "searchLayout"
                view: tree
                model: testModel
            }
            BusyIndicator {
                objectName: "searchSpinner"
                running: testModel.searching || searchLayout.busy
                visible: running
            }
            TextField {
                objectName: "typingField"
                anchors.bottom: parent.bottom
                width: parent.width
                focus: true
            }
        }
    )", QUrl::fromLocalFile(QFileInfo(QString::fromUtf8(__FILE__)).dir().absoluteFilePath("../../qml/TreeSearchHarness.qml")));
    std::unique_ptr<QObject> view(component.create());
    if (!view) std::fprintf(stderr, "%s\n", qPrintable(component.errorString()));
    check(bool(view), "Create real QML TreeView with the search model");
    if (argc == 4 && QString::fromUtf8(argv[1]) == "--ui-benchmark") {
        auto *layout = view->findChild<QObject *>("searchLayout");
        auto *field = view->findChild<QObject *>("typingField");
        QMetaObject::invokeMethod(field, "forceActiveFocus");
        model.setRootPath(QString::fromUtf8(argv[2]));
        QElapsedTimer elapsed, gap;
        elapsed.start(); gap.start();
        qint64 maxGap = 0;
        int keys = 0;
        QTimer input;
        input.setInterval(10);
        QObject::connect(&input, &QTimer::timeout, [&] {
            maxGap = qMax(maxGap, gap.restart());
            QKeyEvent press(QEvent::KeyPress, Qt::Key_X, Qt::NoModifier, "x");
            QKeyEvent release(QEvent::KeyRelease, Qt::Key_X, Qt::NoModifier, "x");
            QCoreApplication::sendEvent(view.get(), &press);
            QCoreApplication::sendEvent(view.get(), &release);
            ++keys;
        });
        input.start();
        model.setSearchText(QString::fromUtf8(argv[3]));
        while (elapsed.elapsed() < 180000) {
            QCoreApplication::processEvents();
            QThread::msleep(2);
            if (elapsed.elapsed() >= 1000 && !model.searching() && layout->property("ready").toBool()
                && (!layout->property("busy").isValid() || !layout->property("busy").toBool())) break;
        }
        check(!model.searching() && !layout->property("busy").toBool(), "UI benchmark completes all result batches");
        check(field->property("text").toString().size() == keys, "No typed keys are lost while results render");
        std::printf("UI search: complete=%lld ms; longest input gap=%lld ms; keys=%d; rows=%d\n",
                    static_cast<long long>(elapsed.elapsed()), static_cast<long long>(maxGap), keys,
                    view->findChild<QObject *>("tree")->property("rows").toInt());
        return 0;
    }
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
    const int iconRole = model.roleNames().key("fileIcon");
    check(iconRole != 0 && album.data(iconRole).toString() == "folder"
              && !song.data(iconRole).toString().isEmpty(),
          "Workers supply folder and MIME icons without delegate filesystem calls");
    auto *tree = view->findChild<QObject *>("tree");
    auto *layout = view->findChild<QObject *>("searchLayout");
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
    waitFor([&] { return layout->property("ready").toBool(); }, "Results become visible after layout frames settle");
    check(tree->property("opacity").toDouble() == 1, "Stable results are visible");
    model.setSearchText(QString::fromUtf8("日本語 theme"));
    check(tree->property("opacity").toDouble() == 0, "Intermediate search/model resets are never painted");
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
    QPersistentModelIndex empty(childNamed(model, matchedAlbum, "Empty"));
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
    check(model.filePath(model.viewRootIndex()) == base.filePath("Other"), "Live filesystem restored");
    for (int i = 0; i < 2010; ++i) {
        QFile file(base.filePath(QString("Other/limit-%1.mid").arg(i)));
        check(file.open(QIODevice::WriteOnly), "Create limit fixture");
    }
    auto *typingField = view->findChild<QObject *>("typingField");
    QMetaObject::invokeMethod(typingField, "forceActiveFocus");
    typingField->setProperty("text", "");
    int typedKeys = 0;
    int partialInputTicks = 0;
    QTimer typing;
    typing.setInterval(1);
    QObject::connect(&typing, &QTimer::timeout, [&] {
        if (model.searching() && model.rowCount(model.viewRootIndex()) > 0)
            ++partialInputTicks;
        QKeyEvent press(QEvent::KeyPress, Qt::Key_X, Qt::NoModifier, "x");
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_X, Qt::NoModifier, "x");
        QCoreApplication::sendEvent(view.get(), &press);
        QCoreApplication::sendEvent(view.get(), &release);
        ++typedKeys;
    });
    typing.start();
    model.setSearchText("limit-");
    settle(model);
    waitFor([&] { return !layout->property("busy").toBool(); }, "Large result layout finishes");
    typing.stop();
    check(partialInputTicks > 1, "Keyboard input runs between partial result insertion batches");
    check(typedKeys > 0 && typingField->property("text").toString().size() == typedKeys,
          "The focused search field accepts every key during a large result update");
    check(model.rowCount(model.viewRootIndex()) == 2000, "Search result bound");
    check(model.searchStatus().contains("narrow"), "Result limit is explained");
    bool replacedPartialQuery = false;
    QTimer replacement;
    replacement.setInterval(1);
    QObject::connect(&replacement, &QTimer::timeout, [&] {
        const int rows = model.rowCount(model.viewRootIndex());
        if (model.searching() && rows > 0 && rows < 2000) {
            replacement.stop();
            replacedPartialQuery = true;
            model.setSearchText("no-such-new-query");
        }
    });
    replacement.start();
    model.setSearchText("limit- ");
    settle(model);
    replacement.stop();
    check(replacedPartialQuery && model.rowCount(model.viewRootIndex()) == 0,
          "A new query interrupts a partially applied snapshot without stale rows");
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
    check(model.filePath(model.viewRootIndex()) == base.absolutePath(), "Clearing search restores the actual browse root");

    const auto zip = base.filePath(QString::fromUtf8("Pack + 日本語.zip"));
    writeArchive(zip);
    waitFor([&] { return childNamed(model, model.viewRootIndex(), QFileInfo(zip).fileName()).isValid(); },
            "Live directory watcher discovers a newly created archive");
    QPersistentModelIndex zipIndex(childNamed(model, model.viewRootIndex(), QFileInfo(zip).fileName()));
    check(model.hasChildren(zipIndex) && model.canFetchMore(zipIndex), "Archives expand in normal browsing");
    model.fetchMore(zipIndex);
    waitFor([&] { return model.rowCount(zipIndex) == 2; }, "Archive exposes implied internal directories");
    QPersistentModelIndex zipDisc(childNamed(model, zipIndex, "Disc"));
    model.fetchMore(zipDisc);
    waitFor([&] { return model.rowCount(zipDisc) == 2; }, "Internal archive directories expand lazily");
    auto encoded = childNamed(model, zipDisc, QString::fromUtf8("日本語 + #%.mid"));
    auto location = kogArchiveLocation(model.filePath(encoded));
    check(location.archive == zip && location.entry == QString::fromUtf8("Disc/日本語 + #%.mid")
              && !location.directory, "Archive identities round-trip Unicode and URL punctuation");
    auto listing = kogListArchive(zip, std::make_shared<std::atomic_bool>(false));
    check(!listing.entries.contains("../escape.mid") && !listing.entries.contains("escape.mid"),
          "Unsafe archive paths never enter the tree");
    model.setSearchText("Hidden Tune");
    settle(model);
    waitFor([&] { return tree->property("rows").toInt() == 3; },
            "Searching inside a closed archive reveals the archive, directory, and matching file");
    auto foundZip = model.index(0, 0, model.viewRootIndex());
    auto foundDisc = model.index(0, 0, foundZip);
    auto foundSong = model.index(0, 0, foundDisc);
    check(kogArchiveLocation(model.filePath(foundSong)).entry == "Disc/Hidden Tune.mid",
          "Search results preserve playable archive-member identities");
    model.setSearchText("Pack");
    settle(model);
    QPersistentModelIndex pack(model.index(0, 0, model.viewRootIndex()));
    check(model.canFetchMore(pack), "An archive-name match remains expandable");
    model.fetchMore(pack);
    waitFor([&] { return model.rowCount(pack) == 2; }, "Browse all contents of an archive-name match");
    const auto sevenZip = base.filePath("compressed.7z");
    writeArchive(sevenZip, true);
    check(kogListArchive(sevenZip, std::make_shared<std::atomic_bool>(false)).entries.contains("Disc/Hidden Tune.mid"),
          "7z archives use the same real in-process reader");
    check(base.mkpath("directory.zip"), "Create a directory with an archive suffix");
    check(!kogIsArchive(base.filePath("directory.zip")), "Real directories are never mistaken for archives");
    QFile broken(base.filePath("broken.zip"));
    check(broken.open(QIODevice::WriteOnly), "Create damaged archive fixture");
    broken.write("not an archive");
    broken.close();
    model.setSearchText("no such song");
    settle(model);
    check(model.searchStatus().contains("could not be searched"), "Unsearchable archives are reported without hanging");
    model.setSearchText("");
    QFile live(base.filePath("Live.mid"));
    check(live.open(QIODevice::WriteOnly), "Create live directory entry");
    live.close();
    waitFor([&] { return childNamed(model, model.viewRootIndex(), "Live.mid").isValid(); },
            "Normal browsing retains live filesystem updates");
    check(live.remove(), "Remove temporary test entry");
    waitFor([&] { return !childNamed(model, model.viewRootIndex(), "Live.mid").isValid(); },
            "Removed files disappear without resetting the whole tree");

    const auto cacheArchive = base.filePath("cache-test.zip");
    writeArchive(cacheArchive);
    const auto notCancelled = std::make_shared<std::atomic_bool>(false);
    const auto cold = kogListArchive(cacheArchive, notCancelled);
    check(!cold.fromCache && cold.error.isEmpty(), "First read builds an archive index");
    check(kogListArchive(cacheArchive, notCancelled).fromCache, "Memory cache reuses the index");
    kogClearArchiveMemoryCache();
    const auto disk = kogListArchive(cacheArchive, notCancelled);
    check(disk.fromCache && disk.entries == cold.entries, "Disk cache survives memory-cache eviction/restarts");
    QFile changed(cacheArchive);
    check(changed.open(QIODevice::WriteOnly | QIODevice::Append), "Modify archive fingerprint");
    changed.write("padding");
    changed.close();
    check(!kogListArchive(cacheArchive, notCancelled).fromCache, "Changed archives invalidate cached indexes");

    // Pace a real cold archive read with a test-only barrier. This verifies
    // streaming and cancellation without relying on storage speed or sleeps.
    QTemporaryDir streaming;
    writeArchive(streaming.filePath("slow.zip"));
    QFile early(streaming.filePath("Hidden Tune.mid"));
    check(early.open(QIODevice::WriteOnly), "Create ordinary early result");
    early.close();
    std::atomic_bool entered{false}, release{false};
    kogSetArchiveReadTestHook([&] {
        entered.store(true);
        while (!release.load()) QThread::msleep(1);
    });
    model.setRootPath(streaming.path());
    model.setSearchText("Hidden Tune");
    waitFor([&] { return entered.load() && model.rowCount(model.viewRootIndex()) == 1
        && layout->property("ready").toBool(); }, "Ordinary matches appear before archive scanning finishes");
    check(model.searching() && tree->property("opacity").toDouble() == 1,
          "Partial results remain visible and interactive during search");
    check(view->findChild<QObject *>("searchSpinner")->property("running").toBool(), "Spinner remains active for partial results");
    check(model.searchStatus().contains("Archives 0 of 1"), "Archive progress is reported");
    QPersistentModelIndex earlyIndex(model.index(0, 0, model.viewRootIndex()));
    release.store(true);
    settle(model);
    kogSetArchiveReadTestHook({});
    check(earlyIndex.isValid() && model.filePath(earlyIndex) == early.fileName(),
          "Streaming archive results preserve existing indexes/selections");
    check(model.rowCount(model.viewRootIndex()) == 2, "Archive matches append to ordinary results");
    waitFor([&] { return !view->findChild<QObject *>("searchSpinner")->property("running").toBool(); },
            "Spinner stops after scanning and incremental layout complete");
    QTemporaryDir branched;
    QDir branches(branched.path());
    for (int i = 0; i < 80; ++i) {
        const auto path = QString("branch-%1/deep").arg(i);
        check(branches.mkpath(path), "Create ancestors spanning insertion batches");
        QFile song(branches.filePath(path + "/needle.mid"));
        check(song.open(QIODevice::WriteOnly), "Create nested search match");
    }
    model.setRootPath(branched.path());
    model.setSearchText("needle");
    settle(model);
    waitFor([&] { return !layout->property("busy").toBool() && layout->property("ready").toBool(); },
            "Batched nested result layout completes");
    check(tree->property("rows").toInt() == 240,
          "Every ancestor expands even when its children arrive in a later batch");
    model.setSearchText("limit"); // Destruction during a scan is safe.
    std::puts("File tree search tests passed");
}
