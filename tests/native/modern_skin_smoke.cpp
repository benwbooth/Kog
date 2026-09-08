#include "kog_modern_skin.h"
#include "kog_file_tree_search.h"
#include "kog_tree_archive.h"

#include <archive.h>
#include <archive_entry.h>
#include <QtCore/QCoreApplication>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QDir>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTemporaryDir>
#include <QtCore/QThread>
#include <QtGui/QImage>
#include <QtGui/QFileSystemModel>
#include <QtQml/QQmlComponent>
#include <QtQml/QQmlEngine>
#include <QtQml/QQmlContext>
#include <QtQml/QJSValue>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickWindow>
#include <QtWebChannelQuick/QQmlWebChannel>
#include <QtTest/QTest>
#include <QtTest/QSignalSpy>
#include <QtWidgets/QApplication>

#include <cstdio>
#include <cstdlib>
#include <functional>
#include <memory>

namespace {
class StatusObserver final : public QObject {
    Q_OBJECT
public:
    QObject *player = nullptr;
public slots:
    void changed() {
        if (player) std::fprintf(stderr, "SKIN STATUS: %s\n", qPrintable(player->property("rendererStatus").toString()));
    }
};

[[noreturn]] void fail(const QString &message)
{
    std::fprintf(stderr, "FAIL: %s\n", qPrintable(message));
    std::exit(1);
}

void require(bool condition, const QString &message)
{
    if (!condition) fail(message);
}

bool waitFor(const std::function<bool()> &condition, int timeoutMs, const QString &what)
{
    QElapsedTimer timer;
    timer.start();
    while (timer.elapsed() < timeoutMs) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
        if (condition()) return true;
        QThread::msleep(10);
    }
    std::fprintf(stderr, "Timed out waiting for %s\n", qPrintable(what));
    return false;
}

class MockApp final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString playback_state READ playbackState NOTIFY stateChanged)
    Q_PROPERTY(double duration_seconds READ durationSeconds NOTIFY stateChanged)
    Q_PROPERTY(int playlist_count READ playlistCount NOTIFY stateChanged)
    Q_PROPERTY(int playlist_revision READ playlistRevision NOTIFY stateChanged)
    Q_PROPERTY(QString directory_path READ directoryPath NOTIFY directory_pathChanged)
public:
    QString playbackState() const { return m_playback; }
    double durationSeconds() const { return 245; }
    int playlistCount() const { return m_secondState ? 2 : 3; }
    int playlistRevision() const { return m_revision; }
    int stateRequests() const { return m_stateRequests; }
    int fullStateRequests() const { return m_fullStateRequests; }
    int nextRequests() const { return m_nextRequests; }
    QString directoryPath() const { return m_directoryPath; }
    const QStringList &addedPaths() const { return m_addedPaths; }
    const QStringList &activatedPaths() const { return m_activatedPaths; }

    void setDirectoryPath(const QString &path)
    {
        if (m_directoryPath == path) return;
        m_directoryPath = path;
        emit directory_pathChanged();
    }

    Q_INVOKABLE QString skin_state(bool includeTracks)
    {
        ++m_stateRequests;
        if (includeTracks) ++m_fullStateRequests;
        QJsonObject state {
            {"playback", m_playback},
            {"position", 42},
            {"duration", durationSeconds()},
            {"volume", 0.6},
            {"currentIndex", 0},
            {"revision", m_revision},
            {"shuffle", "off"},
            {"repeat", "playlist"},
            {"eq", QJsonArray {0, 1, 2, 3, 4, 5, 4, 3, 2, 1}},
            {"eqEnabled", true},
            {"eqPreamp", 2},
            {"visualization", QJsonObject {{"wave", QJsonArray {0, 0.5, -0.5, 0}},
                                             {"spectrum", QJsonArray {0.2, 0.7, 0.4}}}},
        };
        if (includeTracks) {
            QJsonArray tracks;
            if (!m_secondState) {
                tracks.append(QJsonObject {{"id", "one"}, {"title", "First fixture title"},
                                           {"artist", "Fixture Artist"}, {"album", "Smoke Album"},
                                           {"duration", 125}});
            }
            tracks.append(QJsonObject {{"id", "two"}, {"title", "Second fixture title"},
                                       {"artist", "Fixture Artist"}, {"album", "Smoke Album"},
                                       {"duration", 123}});
            tracks.append(QJsonObject {{"id", "three"},
                                       {"title", m_secondState ? "Playlist update" : "Third fixture title"},
                                       {"artist", "Fixture Artist"}, {"album", "Smoke Album"},
                                       {"duration", 122}});
            state["tracks"] = tracks;
        }
        return QString::fromUtf8(QJsonDocument(state).toJson(QJsonDocument::Compact));
    }

    Q_INVOKABLE void play_pause() { m_playback = m_playback == "playing" ? "paused" : "playing"; emit stateChanged(); }
    Q_INVOKABLE void stop() { m_playback = "stopped"; emit stateChanged(); }
    Q_INVOKABLE void next() { ++m_nextRequests; }
    Q_INVOKABLE void previous() {}
    Q_INVOKABLE void seek(double) {}
    Q_INVOKABLE void set_volume_level(double) {}
    Q_INVOKABLE void activate_playlist_index(int) {}
    Q_INVOKABLE void remove_tracks(const QString &) {}
    Q_INVOKABLE void move_tracks(const QString &, int) {}
    Q_INVOKABLE void clear_playlist() {}
    Q_INVOKABLE void open_audio_files() {}
    Q_INVOKABLE void save_playlist() {}
    Q_INVOKABLE void select_shuffle_mode(const QString &) {}
    Q_INVOKABLE void select_repeat_mode(const QString &) {}
    Q_INVOKABLE void update_skin_equalizer_band(int, double) {}
    Q_INVOKABLE void update_equalizer_preamp(double) {}
    Q_INVOKABLE void update_equalizer_enabled(bool) {}
    Q_INVOKABLE void choose_music_folder() { ++m_chooseMusicFolderRequests; }
    Q_INVOKABLE void add_local_path(const QString &path) { m_addedPaths.append(path); }
    Q_INVOKABLE void activate_local_path(const QString &path) { m_activatedPaths.append(path); }

    void advancePlaylist()
    {
        m_secondState = true;
        ++m_revision;
        emit stateChanged();
    }
signals:
    void stateChanged();
    void directory_pathChanged();
private:
    QString m_playback = qEnvironmentVariableIsSet("KOG_MODERN_INSPECT_STOPPED") ? "stopped" : "playing";
    int m_revision = 7;
    int m_stateRequests = 0;
    int m_fullStateRequests = 0;
    int m_nextRequests = 0;
    bool m_secondState = false;
    QString m_directoryPath;
    QStringList m_addedPaths;
    QStringList m_activatedPaths;
    int m_chooseMusicFolderRequests = 0;
};

class MockMainWindow final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool applicationQuitRequested MEMBER applicationQuitRequested)
public:
    Q_INVOKABLE void showFromTray() { ++restoreRequests; }
    int restoreRequests = 0;
    bool applicationQuitRequested = false;
};

void runJavaScript(QObject *view, const QString &script)
{
    const QJSValue callback;
    require(QMetaObject::invokeMethod(view, "runJavaScript", Q_ARG(QString, script), Q_ARG(QJSValue, callback)),
            "invoke WebEngineView.runJavaScript");
}

void checkRenderedTitle(QObject *web, QObject *player, const QString &title)
{
    const QString quoted = QString::fromUtf8(QJsonDocument(QJsonArray {title}).toJson(QJsonDocument::Compact));
    runJavaScript(web, QStringLiteral(
        "(() => { const expected = %1[0].replace(/\\s/g, '').toLowerCase(); let attempts = 0; "
        "const inspect = () => { const ticker = [...document.querySelectorAll('[id]')].find(el => "
        "['songticker', 'm.st.ticker'].includes(el.id.toLowerCase()) && (() => { const r = el.getBoundingClientRect(); "
        "return r.width > 0 && r.height > 0 && r.left >= 0 && r.top >= 0 && r.right <= innerWidth && r.bottom <= innerHeight; })()); "
        "const visible = (ticker?.innerText || '').replace(/\\s/g, '').toLowerCase(); "
        "if (visible.includes(expected)) { window.kogModern.commands.send('error', 'rendered title verified; vu=' + "
        "(window.kogModern.root.audio._vuMeter > 0.1)); } else if (++attempts < 80) { setTimeout(inspect, 100); } "
        "else { window.kogModern.commands.send('error', 'rendered title missing: ' + document.body.innerText.slice(0, 200)); } }; inspect(); })()"
    ).arg(quoted));
    require(waitFor([player] { return player->property("rendererStatus").toString().contains("rendered title verified"); },
                    12'000, "visible skin song title"),
            "skin displays " + title + "; status was: " + player->property("rendererStatus").toString());
    require(player->property("rendererStatus").toString().contains("vu=true"), "MAKI VU meter retains host PCM");
}

void checkAllowlist()
{
    require(kogModernRequestAllowed(QUrl("qrc:/kog/modern/index.html")), "allow bundled renderer URL");
    require(kogModernRequestAllowed(QUrl("qrc:///qtwebchannel/qwebchannel.js")), "allow Qt WebChannel runtime");
    require(kogModernRequestAllowed(QUrl("data:image/png;base64,AA==")), "allow renderer data URL");
    require(kogModernRequestAllowed(QUrl("blob:qrc:/kog/modern/runtime")), "allow renderer blob URL");
    require(kogModernRequestAllowed(QUrl("kogskin://current/skin.wal")), "allow current skin archive URL");
    require(!kogModernRequestAllowed(QUrl("https://example.invalid/skin.wal")), "block network URL");
    require(!kogModernRequestAllowed(QUrl("file:///etc/passwd")), "block filesystem URL");
    require(!kogModernRequestAllowed(QUrl("kogskin://other/skin.wal")), "block other skin authority");
    require(!kogModernRequestAllowed(QUrl("kogskin://current/other.wal")), "block other skin path");
    require(!kogModernRequestAllowed(QUrl("qrc:/not-kog/index.html")), "block unrelated resource URL");
}

void checkNativeWebChannelBoundary(QObject *player, KogFileTreeSearch *libraryModel)
{
    auto *channel = player->findChild<QQmlWebChannel *>();
    require(channel != nullptr, "ModernPlayer created the QML WebChannel");
    const auto objects = static_cast<QWebChannel *>(channel)->registeredObjects();
    require(objects.size() == 1 && objects.contains("kog"), "WebChannel registers only the kog bridge");
    QObject *bridge = objects.value("kog");
    require(bridge != nullptr && bridge != libraryModel, "WebChannel bridge is not the native library model");
    const QStringList forbidden {"libraryModel", "filePath", "setRootPath", "isDir", "directory_path"};
    for (const auto &name : forbidden) {
        require(bridge->metaObject()->indexOfProperty(name.toUtf8().constData()) < 0,
                "WebChannel bridge has no native library property: " + name);
        for (int index = 0; index < bridge->metaObject()->methodCount(); ++index)
            require(!QString::fromLatin1(bridge->metaObject()->method(index).methodSignature()).contains(name,
                    Qt::CaseInsensitive), "WebChannel bridge has no native library API: " + name);
    }
    const QStringList bridgeProperties {"stateJson", "tracksJson", "skinUrl"};
    for (const auto &name : bridgeProperties)
        require(bridge->metaObject()->indexOfProperty(name.toUtf8().constData()) >= 0,
                "WebChannel bridge exposes " + name);
    require(bridge->metaObject()->indexOfMethod("request(QVariant,QVariant)") >= 0
                || bridge->metaObject()->indexOfMethod("request(QString,QString)") >= 0,
            "WebChannel bridge exposes its request command API");
}

void writeArchiveFixture(const QString &path)
{
    auto *writer = archive_write_new();
    require(archive_write_set_format_zip(writer) == ARCHIVE_OK, "create zip archive fixture");
    const auto encoded = QFile::encodeName(path);
    require(archive_write_open_filename(writer, encoded.constData()) == ARCHIVE_OK, "open zip archive fixture");
    auto *entry = archive_entry_new();
    archive_entry_set_pathname_utf8(entry, "Archive Disc/Inside Archive Track.mid");
    archive_entry_set_filetype(entry, AE_IFREG);
    archive_entry_set_perm(entry, 0644);
    archive_entry_set_size(entry, 4);
    require(archive_write_header(writer, entry) >= ARCHIVE_WARN, "write archive fixture header");
    require(archive_write_data(writer, "test", 4) == 4, "write archive fixture data");
    archive_entry_free(entry);
    require(archive_write_close(writer) == ARCHIVE_OK, "close archive fixture");
    archive_write_free(writer);
}

QModelIndex modelIndexForPath(const QAbstractItemModel *model, const QString &path,
                              const QModelIndex &parent = {})
{
    for (int row = 0; row < model->rowCount(parent); ++row) {
        const auto index = model->index(row, 0, parent);
        if (index.data(QFileSystemModel::FilePathRole).toString() == path) return index;
        if (const auto child = modelIndexForPath(model, path, index); child.isValid()) return child;
    }
    return {};
}

QObject *controlWithText(QObject *root, const QString &text)
{
    for (auto *item : root->findChildren<QObject *>()) {
        if (item->property("text").toString() == text && item->metaObject()->indexOfMethod("click()") >= 0)
            return item;
    }
    return nullptr;
}

QQuickItem *quickItemForPath(QQuickItem *root, const QString &path)
{
    for (auto *item : root->childItems()) {
        if (item->property("filePath").toString() == path && item->isVisible()) return item;
        if (auto *nested = quickItemForPath(item, path); nested) return nested;
    }
    return nullptr;
}

void clickControl(QQuickWindow *window, QObject *control, const QString &what)
{
    require(control != nullptr, "find " + what);
    auto *item = qobject_cast<QQuickItem *>(control);
    require(item != nullptr && item->isVisible() && item->isEnabled(), "show enabled " + what);
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                      item->mapToScene({item->width() / 2, item->height() / 2}).toPoint());
}

void invokeModernCommand(QObject *player, const QString &command, const QString &payload)
{
    require(QMetaObject::invokeMethod(player, "command", Q_ARG(QVariant, QVariant(command)),
                                      Q_ARG(QVariant, QVariant(payload))),
            "invoke ModernPlayer command " + command);
}

void checkClassicProBrowserScripts(QObject *player, QObject *web)
{
    runJavaScript(web, QStringLiteral(
        "(() => { const failures = window.kogModern.scriptDiagnostics.filter(message => "
        "message.toLowerCase().includes('browser.maki') || message.includes('ToggleButton.setactivated')); "
        "window.kogModern.commands.send('error', 'modern browser scripts failures=' + JSON.stringify(failures)); })()"));
    require(waitFor([player] {
                return player->property("rendererStatus").toString().contains("modern browser scripts failures=");
            }, 10'000, "ClassicPro browser script diagnostics"), "renderer reports browser script diagnostics");
    require(player->property("rendererStatus").toString() == "Skin error: modern browser scripts failures=[]",
            "ClassicPro browser initialization and callbacks complete: " + player->property("rendererStatus").toString());
    runJavaScript(web, QStringLiteral(
        "(() => { const visited = new Set(); let translated = 0; const failures = []; const visit = object => { "
        "if (!object || visited.has(object)) return; visited.add(object); "
        "if (Number(object._translate) === 2 && object._tooltip?.startsWith('@nullsoft.browser#')) { "
        "const title = object._div.getAttribute('title'); if (title && !title.startsWith('@')) translated++; "
        "else failures.push(object.getId() + ':' + title); } (object._children || []).forEach(visit); }; "
        "window.kogModern.root.getContainers().forEach(container => container._layouts.forEach(visit)); "
        "const sample = window.kogModern.root.vm._scripts[0].variables[0].value.getstring('nullsoft.browser', 17); "
        "window.kogModern.commands.send('error', 'modern translated tooltips=' + translated + '; sample=' + sample + '; failures=' + JSON.stringify(failures.slice(0, 5))); })()"));
    require(waitFor([player] {
                return player->property("rendererStatus").toString().contains("modern translated tooltips=");
            }, 10'000, "translated modern-skin tooltip inspection"), "renderer reports translated tooltips");
    const QString translationStatus = player->property("rendererStatus").toString();
    require(!translationStatus.contains("tooltips=0;") && translationStatus.contains("; sample=Location;")
                && translationStatus.endsWith("failures=[]"),
            "real ClassicPro string-table tooltips render translated text: " + translationStatus);
}

void checkNativeLibraryPanel(QObject *player, QObject *web, KogFileTreeSearch &model,
                             MockApp &app, const QString &libraryRoot, const QString &trackPath,
                             const QString &archivePath, const QString &changedRoot,
                             const QString &changedTrackPath)
{
    auto *window = qobject_cast<QQuickWindow *>(player);
    require(window != nullptr, "ModernPlayer library test has a QQuickWindow");
    auto *loader = player->findChild<QObject *>("modernLibraryLoader");
    require(loader != nullptr, "ModernPlayer created native library loader");
    const bool libraryVisible = waitFor([loader] { return loader->property("visible").toBool(); }, 15'000,
                                        "native library viewport");
    require(libraryVisible, "Cpro library slot displays the trusted native overlay");
    auto *panel = player->findChild<QObject *>("modernLibraryPanel");
    auto *tree = player->findChild<QObject *>("modernLibraryTree");
    auto *search = player->findChild<QObject *>("modernLibrarySearch");
    require(panel != nullptr && panel->property("visible").toBool(), "native library panel is visible");
    require(tree != nullptr && tree->property("visible").toBool() && tree->property("enabled").toBool(),
            "native library TreeView is visible and enabled");
    require(search != nullptr, "native library search field exists");

    const auto tracksPath = QDir(libraryRoot).filePath("tracks");
    require(waitFor([&model, &tracksPath] {
                return modelIndexForPath(&model, tracksPath).isValid();
            }, 10'000, "native model folder scan"),
            "real FileTreeModel lists the fixture tracks folder");

    require(search->setProperty("text", "Inside Archive Track"), "type archive query into native search field");
    const auto archiveEntry = kogArchiveUrl(archivePath, "Archive Disc/Inside Archive Track.mid", false);
    require(waitFor([&model, &archiveEntry] {
                return !model.searching() && modelIndexForPath(&model, archiveEntry).isValid();
            }, 15'000, "native archive search"),
            "native search finds an entry in the real archive fixture");

    require(search->setProperty("text", "Native Library Track"), "type file query into native search field");
    require(waitFor([&model, &trackPath] {
                return !model.searching() && modelIndexForPath(&model, trackPath).isValid();
            }, 15'000, "native file search"),
            "native search finds the real local track");
    require(waitFor([tree] { return tree->property("rows").toInt() >= 2; }, 10'000, "visible native search rows"),
            "TreeView renders the matching folder and local result from the native model");
    require(waitFor([tree] { return tree->property("enabled").toBool() && tree->property("opacity").toReal() > 0.99; },
                    10'000, "settled native search layout"), "native search layout becomes interactive after expanding results");

    // Drive the QML tree row and buttons with pointer clicks, not fixture backend calls.
    require(waitFor([window, &trackPath] { return quickItemForPath(window->contentItem(), trackPath) != nullptr; },
                    10'000, "visible native search result"), "TreeView materializes the local search result");
    clickControl(window, quickItemForPath(window->contentItem(), trackPath), "native search result");
    require(waitFor([panel, &trackPath] { return panel->property("selectedPath").toString() == trackPath; },
                    5'000, "native result selection"), "clicking the native tree selects its path");
    clickControl(window, controlWithText(panel, "Add to playlist"), "Add to playlist button");
    clickControl(window, controlWithText(panel, "Play"), "Play button");
    require(app.addedPaths().contains(trackPath), "native Add to playlist click reaches host add_local_path");
    require(app.activatedPaths().contains(trackPath), "native Play click reaches host activate_local_path");

    app.setDirectoryPath(changedRoot);
    require(waitFor([&model, &changedRoot, &changedTrackPath] {
                return model.filePath(model.viewRootIndex()) == changedRoot
                    && modelIndexForPath(&model, changedTrackPath).isValid();
            }, 10'000, "directory_path native model refresh"),
            "directory_pathChanged refreshes the panel's real native model root");
    require(panel->property("selectedPath").toString().isEmpty(), "directory change clears the native tree selection");

    // The native library is a QML overlay rather than DOM/WebChannel content. Do not reconnect a
    // second QWebChannel here: Qt's transport has one active response dispatcher.
    runJavaScript(web, QStringLiteral(
        "(() => { const forbidden = ['libraryModel', 'filePath', 'setRootPath', 'isDir', 'directory_path']; "
        "const surface = [window.kogModern, window.kogModern.commands, window.kogModern.state]; "
        "const isolated = forbidden.every(name => surface.every(value => !(name in value))) "
        "&& !document.body.innerText.includes('Native Library Track'); "
        "window.kogModern.commands.send('error', 'library renderer isolated=' + isolated); })()"));
    require(waitFor([player] { return player->property("rendererStatus").toString().contains("library renderer isolated=true"); },
                    10'000, "library renderer isolation"), "renderer cannot reach native model or filesystem APIs");

    // Invalid geometry is rejected by the QML command boundary, then the real renderer republishes its slot.
    invokeModernCommand(player, "libraryViewport", R"({"x":-1,"y":0,"width":50,"height":50})");
    require(!loader->property("visible").toBool(), "out-of-bounds renderer geometry immediately hides the native overlay");
    require(waitFor([loader] { return loader->property("visible").toBool(); }, 5'000, "republished library viewport"),
            "renderer restores the native overlay from its real library slot");

    // This is the exact geometry-only notification published when the skin leaves its library tab.
    invokeModernCommand(player, "libraryViewport", "null");
    require(!loader->property("visible").toBool(), "native overlay immediately hides when the renderer reports no library slot");
}
} // namespace

int main(int argc, char **argv)
{
    if (argc != 4) fail("usage: modern-skin-smoke REPOSITORY SKIN_ARCHIVE SCREENSHOT_PATH");
    const QString repository = QString::fromLocal8Bit(argv[1]);
    const QString skinArchive = QString::fromLocal8Bit(argv[2]);
    const QString screenshotPath = QString::fromLocal8Bit(argv[3]);
    require(QFile::exists(skinArchive), "real modern-skin archive exists");
    const bool expectLibrary = qEnvironmentVariableIntValue("KOG_MODERN_EXPECT_LIBRARY") != 0;

    checkAllowlist();
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    kogInitializeModernSkins();
    QApplication application(argc, argv);
    kogRegisterModernSkinTypes();

    require(QFile::exists(":/kog/modern/index.html"), "compiled modern renderer index exists");
    require(QFile::exists(":/kog/modern/runtime.js"), "compiled modern renderer bundle exists");

    QTemporaryDir libraryFixture;
    require(libraryFixture.isValid(), "create temporary native library fixture");
    QDir libraryRoot(libraryFixture.path());
    require(libraryRoot.mkpath("tracks"), "create fixture tracks folder");
    const QString trackPath = libraryRoot.filePath("tracks/Native Library Track.mid");
    QFile track(trackPath);
    require(track.open(QIODevice::WriteOnly), "create fixture local track");
    require(track.write("test") == 4, "write fixture local track");
    track.close();
    const QString archivePath = libraryRoot.filePath("tracks/fixture.zip");
    writeArchiveFixture(archivePath);
    QTemporaryDir changedLibraryFixture;
    require(changedLibraryFixture.isValid(), "create changed native library fixture");
    QDir changedLibraryRoot(changedLibraryFixture.path());
    require(changedLibraryRoot.mkpath("tracks"), "create changed fixture tracks folder");
    const QString changedTrackPath = changedLibraryRoot.filePath("tracks/Native Library Track replacement.mid");
    QFile changedTrack(changedTrackPath);
    require(changedTrack.open(QIODevice::WriteOnly), "create changed fixture local track");
    require(changedTrack.write("test") == 4, "write changed fixture local track");
    changedTrack.close();

    MockApp app;
    app.setDirectoryPath(libraryRoot.absolutePath());
    MockMainWindow mainWindow;
    KogFileTreeSearch libraryModel;
    QQmlEngine engine;
    QQmlComponent component(&engine, QUrl::fromLocalFile(repository + "/qml/ModernPlayer.qml"));
    require(component.isReady(), "load ModernPlayer.qml: " + component.errorString());
    QVariantMap properties {
        {"app", QVariant::fromValue(&app)},
        {"mainWindow", QVariant::fromValue(&mainWindow)},
        {"skin", QVariantMap {{"title", "MMD3 native smoke"}, {"archivePath", skinArchive}}},
        {"libraryModel", QVariant::fromValue(&libraryModel)},
        {"visible", true},
    };
    std::unique_ptr<QObject> player(component.createWithInitialProperties(properties));
    require(bool(player), "create ModernPlayer.qml: " + component.errorString());
    StatusObserver observer;
    if (qEnvironmentVariableIsSet("KOG_MODERN_DUMP_DIAGNOSTICS")) {
        observer.player = player.get();
        QObject::connect(player.get(), SIGNAL(rendererStatusChanged()), &observer, SLOT(changed()));
    }

    auto *profile = player->findChild<KogModernProfile *>();
    require(profile != nullptr, "ModernPlayer created KogModernProfile");
    require(profile->skinPath() == skinArchive, "ModernPlayer passed archive path to profile");
    auto *web = player->findChild<QObject *>("modernWebView");
    require(web != nullptr, "ModernPlayer created WebEngine view");
    checkNativeWebChannelBoundary(player.get(), &libraryModel);
    auto *window = qobject_cast<QQuickWindow *>(player.get());
    require(window != nullptr, "ModernPlayer is a QQuickWindow");

    const QString expectedError = qEnvironmentVariable("KOG_MODERN_EXPECT_ERROR");
    if (!expectedError.isEmpty()) {
        require(waitFor([&player, &expectedError] {
                    return player->property("rendererStatus").toString().contains(expectedError);
                }, 60'000, "unsupported skin diagnostic"),
                "unsupported skin reports its missing dependency");
        runJavaScript(web, QStringLiteral(
            "new QWebChannel(qt.webChannelTransport, channel => { const status = document.getElementById('runtime-status'); "
            "channel.objects.kog.request('error', JSON.stringify('diagnostic visible=' + "
            "!!(status && !status.hidden && status.classList.contains('fatal') && status.innerText.includes('ClassicPro')))); });"));
        require(waitFor([&player] {
                    return player->property("rendererStatus").toString().contains("diagnostic visible=true");
                }, 10'000, "visible unsupported skin diagnostic"), "failure is visible in the renderer");
        require(window->grabWindow().save(screenshotPath), "capture unsupported skin diagnostic");
        std::printf("Unsupported modern skin diagnostic passed; screenshot: %s\n", qPrintable(screenshotPath));
        return 0;
    }

    require(waitFor([&player] {
                const auto status = player->property("rendererStatus").toString();
                return status.startsWith("Experimental modern skin") || status.startsWith("Skin error:");
            }, 60'000, "renderer ready command"),
            "renderer sent ready through WebChannel; status was: " + player->property("rendererStatus").toString());
    require(player->property("rendererStatus").toString().startsWith("Experimental modern skin"),
            "renderer loaded successfully: " + player->property("rendererStatus").toString());
    require(app.fullStateRequests() > 0, "ready command requested playlist-bearing host state");
    QElapsedTimer settle;
    settle.start();
    while (settle.elapsed() < 3'000) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
        QThread::msleep(10);
    }
    const QImage screenshot = window->grabWindow();
    require(!screenshot.isNull() && screenshot.save(screenshotPath), "capture modern-skin screenshot");
    if (qEnvironmentVariableIsSet("KOG_MODERN_INSPECT_STOPPED")) {
        app.stop();
        QTest::qWait(1200);
        const QImage stopped = window->grabWindow();
        require(stopped.save(screenshotPath + ".stopped.png"), "capture stopped display");
        int magenta = 0;
        for (int y = 37; y < 68; ++y) for (int x = 16; x < 126; ++x) {
            const QColor pixel = stopped.pixelColor(x, y);
            if (pixel.red() > 220 && pixel.green() < 40 && pixel.blue() > 100) ++magenta;
        }
        require(magenta == 0, "stopped timer must not tile the number atlas warning into blank glyphs");
        runJavaScript(web, QStringLiteral(
            "window.kogModern.commands.send('error','clock healthy='+!window.kogModern.scriptDiagnostics.some(m=>m.includes('cProClock')));"));
        require(waitFor([&player] { return player->property("rendererStatus").toString().contains("clock healthy=true"); },
                        10'000, "stopped clock script"), "stopped clock script completes without missing methods");
    }
    if (expectLibrary) {
    auto *resizeGrip = player->findChild<QObject *>("skinResizeGrip");
    require(resizeGrip != nullptr, "modern native resize grip exists");
    QSignalSpy resizeRequests(resizeGrip, SIGNAL(resizeRequested(int)));
    require(resizeRequests.isValid(), "native resize signal exists");
    QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, QPoint(window->width() - 7, window->height() - 7));
    QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier, QPoint(window->width() - 7, window->height() - 7));
    require(resizeRequests.count() == 1 && resizeRequests.at(0).at(0).toInt() == (Qt::RightEdge | Qt::BottomEdge),
            "real corner press synchronously reaches native bottom-right resize");
    QSignalSpy moveRequests(player.get(), SIGNAL(systemMoveRequested()));
    require(moveRequests.isValid(), "native title-bar move signal exists");
    QTest::mousePress(window, Qt::LeftButton, Qt::NoModifier, QPoint(100, 10));
    const bool moveReachedHost = waitFor([&moveRequests] { return !moveRequests.isEmpty(); }, 5'000, "real skin title-bar press");
    QTest::mouseRelease(window, Qt::LeftButton, Qt::NoModifier, QPoint(100, 10));
    require(moveReachedHost, "real title-bar press reaches native system move without dragging the DOM");
    require(window->flags().testFlag(Qt::FramelessWindowHint), "single-window skin owns the native window chrome");
    window->resize(840, 640);
    QTest::qWait(500);
    runJavaScript(web, QStringLiteral(
        "{const l=window.kogModern.root.findContainer('main').getcurlayout(); window.kogModern.commands.send('error','host resize='+l.getwidth()+'x'+l.getheight());}"));
    require(waitFor([&player] { return player->property("rendererStatus").toString().contains("host resize=840x640"); },
                    10'000, "host resize reaches MAKI"), "main layout follows native window resize");
    }
    if (qEnvironmentVariableIsSet("KOG_MODERN_DUMP_DIAGNOSTICS")) {
        runJavaScript(web, QStringLiteral("if (window.kogModern) window.kogModern.scriptDiagnostics.forEach((message, index) => window.kogModern.commands.send('error', 'MAKI ' + index + ': ' + message)); else new QWebChannel(qt.webChannelTransport, channel => channel.objects.kog.request('error', JSON.stringify('Runtime not ready: ' + document.getElementById('runtime-status')?.textContent)));"));
        QCoreApplication::processEvents(QEventLoop::AllEvents, 100);
    }
    runJavaScript(web, QStringLiteral(
        "window.kogModern.commands.send('next'); (() => { let attempts = 0; const inspect = () => { try { "
        "const state = window.kogModern.state.state; if (state.tracks.length && state.tracks[0]) { "
        "window.kogModern.commands.send('error', 'modern smoke state: loaded=' + !!window.kogModern + '; tracks=' + state.tracks.length + "
        "'; title=' + state.tracks[0].title + '; artist=' + state.tracks[0].artist); } else if (++attempts < 50) { "
        "setTimeout(inspect, 100); } else { window.kogModern.commands.send('error', 'modern smoke state did not receive tracks'); } "
        "} catch (error) { window.kogModern.commands.send('error', 'modern smoke inspect failed: ' + error); } }; inspect(); })()"));
    require(waitFor([&app] { return app.nextRequests() == 1; }, 10'000, "renderer command through WebChannel"),
            "renderer command reached the fixture host");
    require(waitFor([&player] { return player->property("rendererStatus").toString().contains("First fixture title"); },
                    10'000, "renderer state inspection"),
            "renderer state inspection completed; status was: " + player->property("rendererStatus").toString());
    const QString initialState = player->property("rendererStatus").toString();
    require(initialState.contains("loaded=true") && initialState.contains("tracks=3"), "renderer received fixture playlist");
    require(initialState.contains("First fixture title") && initialState.contains("Fixture Artist"),
            "renderer received fixture metadata");
    checkRenderedTitle(web, player.get(), "First fixture title");

    const int stateRequestsBeforeUpdate = app.stateRequests();
    app.advancePlaylist();
    require(waitFor([&app, stateRequestsBeforeUpdate] { return app.stateRequests() > stateRequestsBeforeUpdate; },
                    10'000, "playlist revision update"),
            "ModernPlayer requested changed playlist state");
    settle.restart();
    while (settle.elapsed() < 500) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
        QThread::msleep(10);
    }
    runJavaScript(web, QStringLiteral(
        "(() => { let attempts = 0; const inspect = () => { const state = window.kogModern.state.state; "
        "if (state.revision === 8 && state.tracks.length === 2 && state.tracks[1]) { "
        "window.kogModern.commands.send('error', 'modern smoke update: revision=' + state.revision + '; tracks=' + state.tracks.length + "
        "'; title=' + state.tracks[1].title); } else if (++attempts < 50) { setTimeout(inspect, 100); } else { "
        "window.kogModern.commands.send('error', 'modern smoke update did not arrive'); } }; inspect(); })()"));
    require(waitFor([&player] { return player->property("rendererStatus").toString().contains("Playlist update"); },
                    10'000, "revised renderer state inspection"), "revised renderer state inspection completed");
    const QString updatedState = player->property("rendererStatus").toString();
    require(updatedState.contains("revision=8") && updatedState.contains("tracks=2")
                && updatedState.contains("Playlist update"),
            "renderer applied revised playlist and metadata");
    checkRenderedTitle(web, player.get(), "Second fixture title");

    if (expectLibrary) {
        checkNativeLibraryPanel(player.get(), web, libraryModel, app, libraryRoot.absolutePath(), trackPath, archivePath,
                                changedLibraryRoot.absolutePath(), changedTrackPath);
        checkClassicProBrowserScripts(player.get(), web);
        // Exercise the real skin tab, not a synthetic libraryViewport hide.
        QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(132, 116));
        QTest::qWait(700);
        require(window->grabWindow().save(screenshotPath + ".playlist.png"), "capture real Playlist tab");
        runJavaScript(web, QStringLiteral(
            "{ const lists=Array.from(document.querySelectorAll('.content-list')).filter(e=>e.getBoundingClientRect().width>500 && e.innerText.includes('fixture')); const f=window.kogModern.root.findContainer('main').getcurlayout().findobject('centro.mainframe'); window.kogModern.commands.send('error','main playlist visible='+lists.length+'; side='+f.getposition()); }"));
        require(waitFor([&player] { return player->property("rendererStatus").toString().contains("main playlist visible=1"); },
                        10'000, "ClassicPro Playlist tab"), "Playlist tab displays the host playlist in the main pane");
        require(player->property("rendererStatus").toString().endsWith("side=0"), "Playlist tab collapses the duplicate side playlist");
        for (const QSize size : {QSize(500, 500), QSize(840, 640)}) {
            window->resize(size);
            QTest::qWait(500);
            runJavaScript(web, QStringLiteral(
                "{const f=window.kogModern.root.findContainer('main').getcurlayout().findobject('centro.mainframe');"
                "const buttons=Array.from(document.querySelectorAll('button.wasabi')).filter(e=>e.innerText==='Search' && e.getBoundingClientRect().width>0);"
                "const fits=buttons.length>0 && buttons.every(e=>e.scrollWidth<=e.clientWidth && e.getBoundingClientRect().right<=innerWidth);"
                "window.kogModern.commands.send('error','playlist resize healthy='+(f.getposition()===0 && fits));}"));
            require(waitFor([&player] { return player->property("rendererStatus").toString().contains("playlist resize healthy=true"); },
                            10'000, "playlist search geometry"), "playlist remains collapsed and Search fits after shrinking and growing");
            player->setProperty("rendererStatus", QStringLiteral("checking next size"));
        }
    }

    const QString secondSkin = repository + "/native/webamp/packages/webamp-modern/assets/skins/WinampModern566.wal";
    require(QFile::exists(secondSkin), "second real modern-skin archive exists");
    require(player->setProperty("skin", QVariantMap {{"title", "Winamp modern reload smoke"},
                                                       {"archivePath", secondSkin}}),
            "replace ModernPlayer skin fixture");
    if (expectLibrary) {
        auto *loader = player->findChild<QObject *>("modernLibraryLoader");
        require(loader != nullptr && !loader->property("visible").toBool(),
                "skin reload immediately hides the stale native library viewport");
    }
    require(waitFor([profile, &player, &secondSkin] {
                return profile->skinPath() == secondSkin
                    && player->property("rendererStatus").toString().startsWith("Experimental modern skin");
            }, 60'000, "second skin reload ready command"),
            "ModernPlayer reloaded the real second skin; status was: " + player->property("rendererStatus").toString());

    window->close();
    require(mainWindow.restoreRequests == 1 && !window->isVisible(), "ordinary close restores main player");
    window->show();
    mainWindow.applicationQuitRequested = true;
    require(window->close(), "modern window accepts application shutdown");
    require(mainWindow.restoreRequests == 1, "application shutdown does not restore main player");

    std::printf("Modern skin smoke passed; screenshot: %s\n", qPrintable(screenshotPath));
    return 0;
}

#include "modern_skin_smoke.moc"
