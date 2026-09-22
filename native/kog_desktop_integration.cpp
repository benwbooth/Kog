#include "kog_desktop_integration.h"
#include "kog_modern_skin.h"

#include <QtCore/QFileInfo>
#include <QtCore/QHash>
#include <QtCore/QMimeDatabase>
#include <QtCore/QSettings>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QWindow>

#include <array>

std::unique_ptr<QApplication> kogApplicationNew()
{
    kogInitializeModernSkins();
    static QByteArray executableName("kog");
    static QByteArray sessionOption("-session");
    static QByteArray sessionId;
    static std::array<char *, 4> arguments { executableName.data(), nullptr, nullptr, nullptr };
    static int argumentCount = 1;
#ifdef KOG_WAYLAND_SESSION_RESTORE
    if (qEnvironmentVariable("XDG_SESSION_TYPE") == QStringLiteral("wayland")
        && !qEnvironmentVariable("QT_QPA_PLATFORM").startsWith(QStringLiteral("xcb"))) {
        sessionId = QSettings("Kog", "Kog").value("MainWindow/waylandSessionId").toByteArray();
        if (!sessionId.isEmpty()) {
            arguments[1] = sessionOption.data();
            arguments[2] = sessionId.data();
            argumentCount = 3;
        }
    }
#endif
    auto application = std::make_unique<QApplication>(argumentCount, arguments.data());
    kogRegisterModernSkinTypes();
    application->setOrganizationName(QStringLiteral("Kog"));
#ifdef KOG_WAYLAND_SESSION_RESTORE
    if (application->platformName().startsWith(QStringLiteral("wayland"))
        && !application->sessionId().isEmpty()) {
        QSettings settings("Kog", "Kog");
        settings.setValue("MainWindow/waylandSessionId", application->sessionId());
        settings.sync();
    }
#endif
    return application;
}

void kogApplicationSetVersion(QApplication &application, const QString &version)
{
    application.setApplicationVersion(version);
}

void kogApplicationSetName(QApplication &application, const QString &name)
{
    application.setApplicationName(name);
}

int kogApplicationExec(QApplication &application)
{
    return application.exec();
}

QString kogFormatIconName(const QString &suffix)
{
    // Extension families below mirror the decoder backends' static
    // allow-lists (crates/kog-audio/src/*decoder*.rs). Tracker/MIDI/audio
    // membership follows backend priority: an extension handled by an
    // earlier backend belongs to that family (mus/xmf are MIDI, dsf is
    // Saturn, mdz-style tracker archives are plain archives).
    static const auto *keys = new QHash<QString, QString>{
        {"gbs", "gameboy"},
        {"nsf", "nes"},
        {"nsfe", "nes"},
        {"spc", "snes"},
        {"snsf", "snes"},
        {"minisnsf", "snes"},
        {"gsf", "gba"},
        {"minigsf", "gba"},
        {"2sf", "ds"},
        {"mini2sf", "ds"},
        {"ncsf", "ds"},
        {"minincsf", "ds"},
        {"psf", "psx"},
        {"minipsf", "psx"},
        {"psf2", "ps2"},
        {"minipsf2", "ps2"},
        {"ssf", "saturn"},
        {"minissf", "saturn"},
        {"dsf", "saturn"},
        {"minidsf", "saturn"},
        {"usf", "n64"},
        {"miniusf", "n64"},
        {"qsf", "arcade"},
        {"miniqsf", "arcade"},
        {"kss", "msx"},
        {"hes", "pcengine"},
        {"ay", "spectrum"},
        {"sap", "atari"},
        {"sid", "c64"},
        {"hvl", "amiga"},
        {"ahx", "amiga"},
        {"vgm", "chip"},
        {"vgz", "chip"},
        {"gym", "chip"},
        {"s98", "chip"},
        {"dro", "chip"},
        {"sfm", "chip"},
        {"mptm", "tracker"},
        {"mod", "tracker"},
        {"s3m", "tracker"},
        {"xm", "tracker"},
        {"it", "tracker"},
        {"667", "tracker"},
        {"669", "tracker"},
        {"amf", "tracker"},
        {"ams", "tracker"},
        {"c67", "tracker"},
        {"cba", "tracker"},
        {"dbm", "tracker"},
        {"digi", "tracker"},
        {"dmf", "tracker"},
        {"dsm", "tracker"},
        {"dsym", "tracker"},
        {"dtm", "tracker"},
        {"etx", "tracker"},
        {"far", "tracker"},
        {"fc", "tracker"},
        {"fc13", "tracker"},
        {"fc14", "tracker"},
        {"fmt", "tracker"},
        {"fst", "tracker"},
        {"ftm", "tracker"},
        {"imf", "tracker"},
        {"ims", "tracker"},
        {"ice", "tracker"},
        {"j2b", "tracker"},
        {"m15", "tracker"},
        {"mdl", "tracker"},
        {"med", "tracker"},
        {"mms", "tracker"},
        {"mt2", "tracker"},
        {"mtm", "tracker"},
        {"nst", "tracker"},
        {"okt", "tracker"},
        {"plm", "tracker"},
        {"psm", "tracker"},
        {"pt36", "tracker"},
        {"ptm", "tracker"},
        {"puma", "tracker"},
        {"rtm", "tracker"},
        {"sfx", "tracker"},
        {"sfx2", "tracker"},
        {"smod", "tracker"},
        {"st26", "tracker"},
        {"stk", "tracker"},
        {"stm", "tracker"},
        {"stx", "tracker"},
        {"stp", "tracker"},
        {"symmod", "tracker"},
        {"tcb", "tracker"},
        {"gmc", "tracker"},
        {"gtk", "tracker"},
        {"gt2", "tracker"},
        {"ult", "tracker"},
        {"unic", "tracker"},
        {"wow", "tracker"},
        {"gdm", "tracker"},
        {"mo3", "tracker"},
        {"oxm", "tracker"},
        {"umx", "tracker"},
        {"xpk", "tracker"},
        {"ppm", "tracker"},
        {"mmcmp", "tracker"},
        {"org", "tracker"},
        {"jxs", "tracker"},
        {"kar", "midi"},
        {"mid", "midi"},
        {"midi", "midi"},
        {"rmi", "midi"},
        {"mids", "midi"},
        {"mds", "midi"},
        {"lds", "midi"},
        {"xmf", "midi"},
        {"mxmf", "midi"},
        {"hmi", "midi"},
        {"hmp", "midi"},
        {"hmq", "midi"},
        {"mus", "midi"},
        {"xmi", "midi"},
        {"aac", "audio"},
        {"adts", "audio"},
        {"aif", "audio"},
        {"aifc", "audio"},
        {"aiff", "audio"},
        {"alac", "audio"},
        {"caf", "audio"},
        {"flac", "audio"},
        {"m4a", "audio"},
        {"m4b", "audio"},
        {"mka", "audio"},
        {"mkv", "audio"},
        {"mp1", "audio"},
        {"mp2", "audio"},
        {"mp3", "audio"},
        {"mp4", "audio"},
        {"oga", "audio"},
        {"ogg", "audio"},
        {"ogv", "audio"},
        {"opus", "audio"},
        {"wav", "audio"},
        {"wave", "audio"},
        {"webm", "audio"},
        {"wma", "audio"},
        {"asf", "audio"},
        {"tak", "audio"},
        {"m4r", "audio"},
        {"m2a", "audio"},
        {"mpa", "audio"},
        {"ape", "audio"},
        {"ac3", "audio"},
        {"dts", "audio"},
        {"dtshd", "audio"},
        {"tta", "audio"},
        {"vqf", "audio"},
        {"vqe", "audio"},
        {"vql", "audio"},
        {"ra", "audio"},
        {"rm", "audio"},
        {"rmj", "audio"},
        {"weba", "audio"},
        {"dsdiff", "audio"},
        {"dff", "audio"},
        {"wsd", "audio"},
        {"wv", "audio"},
        {"wvp", "audio"},
        {"mpc", "audio"},
        {"shn", "audio"},
        {"iff", "audio"},
        {"apl", "audio"},
        {"zip", "archive"},
        {"rar", "archive"},
        {"7z", "archive"},
        {"rsn", "archive"},
        {"vgm7z", "archive"},
        {"gz", "archive"},
        {"mdz", "archive"},
        {"mdr", "archive"},
        {"s3z", "archive"},
        {"xmz", "archive"},
        {"itz", "archive"},
        {"mptmz", "archive"},
        {"m3u", "playlist"},
        {"m3u8", "playlist"},
        {"pls", "playlist"},
        {"cue", "cue"},
    };
    const auto found = keys->constFind(suffix);
    if (found == keys->cend()) {
        return {};
    }
    return QStringLiteral("kog-format-") + *found;
}

namespace {
// The system theme answer, kept for names Kog has no art for (suffix-less
// files). Everything else resolves through the format table above.
QString kogMimeIconName(const QFileInfo &fileInfo)
{
    const QMimeDatabase database;
    const auto mimeType = database.mimeTypeForFile(fileInfo, QMimeDatabase::MatchExtension);
    auto iconName = mimeType.iconName();
    if (iconName.isEmpty()) {
        iconName = mimeType.genericIconName();
    }
    if (iconName.isEmpty()) {
        iconName = mimeType.name().startsWith(QStringLiteral("audio/"))
            ? QStringLiteral("audio-x-generic")
            : QStringLiteral("text-x-generic");
    }
    return iconName;
}
} // namespace

QString kogFileIconName(const QString &path)
{
    const QFileInfo fileInfo(path);
    if (fileInfo.isDir()) {
        return QStringLiteral("folder");
    }

    // Every caller lists playable files (the tree, the playlist pane), so a
    // known suffix takes Kog's own art and any other playable suffix gets the
    // paper badge; only a suffix-less name falls through to the system theme.
    // Cached per suffix: playlist scrolling resolves hundreds of rows.
    thread_local QHash<QString, QString> icons;
    const auto suffix = fileInfo.suffix().toLower();
    if (suffix.isEmpty()) {
        return kogMimeIconName(fileInfo);
    }
    const auto found = icons.constFind(suffix);
    if (found != icons.cend()) {
        return *found;
    }
    auto iconName = kogFormatIconName(suffix);
    if (iconName.isEmpty()) {
        iconName = QStringLiteral("kog-format-paper");
    }
    icons.insert(suffix, iconName);
    return iconName;
}

void kogApplyApplicationIcon()
{
    const QIcon icon(QStringLiteral(":/qt/qml/org/kog/player/qml/icons/kog.svg"));
    if (icon.isNull()) {
        return;
    }

    QGuiApplication::setWindowIcon(icon);
    for (QWindow *window : QGuiApplication::allWindows()) {
        window->setIcon(icon);
    }
}
