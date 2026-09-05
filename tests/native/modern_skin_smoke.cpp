#include "kog_modern_skin.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QThread>
#include <QtGui/QImage>
#include <QtQml/QQmlComponent>
#include <QtQml/QQmlEngine>
#include <QtQml/QJSValue>
#include <QtQuick/QQuickWindow>
#include <QtWidgets/QApplication>

#include <cstdio>
#include <cstdlib>
#include <functional>
#include <memory>

namespace {
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
public:
    QString playbackState() const { return m_playback; }
    double durationSeconds() const { return 245; }
    int playlistCount() const { return m_secondState ? 2 : 3; }
    int playlistRevision() const { return m_revision; }
    int stateRequests() const { return m_stateRequests; }
    int fullStateRequests() const { return m_fullStateRequests; }
    int nextRequests() const { return m_nextRequests; }

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

    void advancePlaylist()
    {
        m_secondState = true;
        ++m_revision;
        emit stateChanged();
    }
signals:
    void stateChanged();
private:
    QString m_playback = "playing";
    int m_revision = 7;
    int m_stateRequests = 0;
    int m_fullStateRequests = 0;
    int m_nextRequests = 0;
    bool m_secondState = false;
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
        "el.id.toLowerCase() === 'songticker' && el.getBoundingClientRect().width > 0); "
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
} // namespace

int main(int argc, char **argv)
{
    if (argc != 4) fail("usage: modern-skin-smoke REPOSITORY SKIN_ARCHIVE SCREENSHOT_PATH");
    const QString repository = QString::fromLocal8Bit(argv[1]);
    const QString skinArchive = QString::fromLocal8Bit(argv[2]);
    const QString screenshotPath = QString::fromLocal8Bit(argv[3]);
    require(QFile::exists(skinArchive), "real modern-skin archive exists");

    checkAllowlist();
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    kogInitializeModernSkins();
    QApplication application(argc, argv);
    kogRegisterModernSkinTypes();

    require(QFile::exists(":/kog/modern/index.html"), "compiled modern renderer index exists");
    require(QFile::exists(":/kog/modern/runtime.js"), "compiled modern renderer bundle exists");

    MockApp app;
    MockMainWindow mainWindow;
    QQmlEngine engine;
    QQmlComponent component(&engine, QUrl::fromLocalFile(repository + "/qml/ModernPlayer.qml"));
    require(component.isReady(), "load ModernPlayer.qml: " + component.errorString());
    QVariantMap properties {
        {"app", QVariant::fromValue(&app)},
        {"mainWindow", QVariant::fromValue(&mainWindow)},
        {"skin", QVariantMap {{"title", "MMD3 native smoke"}, {"archivePath", skinArchive}}},
        {"visible", true},
    };
    std::unique_ptr<QObject> player(component.createWithInitialProperties(properties));
    require(bool(player), "create ModernPlayer.qml: " + component.errorString());

    auto *profile = player->findChild<KogModernProfile *>();
    require(profile != nullptr, "ModernPlayer created KogModernProfile");
    require(profile->skinPath() == skinArchive, "ModernPlayer passed archive path to profile");
    auto *web = player->findChild<QObject *>("modernWebView");
    require(web != nullptr, "ModernPlayer created WebEngine view");
    auto *window = qobject_cast<QQuickWindow *>(player.get());
    require(window != nullptr, "ModernPlayer is a QQuickWindow");

    require(waitFor([&player] {
                return player->property("rendererStatus").toString().startsWith("Experimental modern skin");
            }, 60'000, "renderer ready command"),
            "renderer sent ready through WebChannel; status was: " + player->property("rendererStatus").toString());
    require(app.fullStateRequests() > 0, "ready command requested playlist-bearing host state");
    QElapsedTimer settle;
    settle.start();
    while (settle.elapsed() < 3'000) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
        QThread::msleep(10);
    }
    const QImage screenshot = window->grabWindow();
    require(!screenshot.isNull() && screenshot.save(screenshotPath), "capture modern-skin screenshot");
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

    const QString secondSkin = repository + "/native/webamp/packages/webamp-modern/assets/skins/WinampModern566.wal";
    require(QFile::exists(secondSkin), "second real modern-skin archive exists");
    require(player->setProperty("skin", QVariantMap {{"title", "Winamp modern reload smoke"},
                                                       {"archivePath", secondSkin}}),
            "replace ModernPlayer skin fixture");
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
