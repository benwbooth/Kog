#include "kog_desktop_integration.h"
#include "kog_modern_skin.h"

#include <QtCore/QDir>
#include <QtCore/QFileInfo>
#include <QtCore/QHash>
#include <QtCore/QMimeDatabase>
#include <QtCore/QSettings>
#include "kog_settings.h"
#include <QtGui/QCursor>
#include <QtGui/QFontDatabase>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QPalette>
#include <QtGui/QStyleHints>
#include <QtGui/QWindow>
#include <QtQml/qqml.h>
#include <QtQuick/QQuickWindow>

#include <array>

namespace {
// Earshot's look with a blue accent: true black in dark mode (for OLED
// screens), a blue accent with a violet second accent. Selection uses a
// deeper blue so white text on it stays legible; the brighter blue is the
// accent (slider fills, focus, links).
struct KogColors {
    QColor window, base, alternateBase, button, text, muted, faint, border,
        light, dark, highlight, accent, accent2, link, tooltip;
};

const KogColors &kogEarshotColors(bool light)
{
    static const KogColors dark {
        QColor(0x00, 0x00, 0x00), QColor(0x00, 0x00, 0x00), QColor(0x0a, 0x0b, 0x0f),
        QColor(0x14, 0x16, 0x1d), QColor(0xf2, 0xf3, 0xf7), QColor(0x9a, 0xa1, 0xb5),
        QColor(0x63, 0x6a, 0x80), QColor(0x1e, 0x22, 0x30), QColor(0x23, 0x28, 0x3a),
        QColor(0x00, 0x00, 0x00), QColor(0x2f, 0x7d, 0xe1), QColor(0x4d, 0x9f, 0xff),
        QColor(0x7a, 0x5c, 0xff), QColor(0x6d, 0xb1, 0xff), QColor(0x14, 0x16, 0x1d),
    };
    static const KogColors bright {
        QColor(0xf1, 0xf1, 0xf7), QColor(0xfb, 0xfb, 0xfe), QColor(0xf2, 0xf2, 0xf8),
        QColor(0xe8, 0xe8, 0xf1), QColor(0x16, 0x18, 0x22), QColor(0x5d, 0x64, 0x78),
        QColor(0x8a, 0x90, 0xa3), QColor(0xd6, 0xd8, 0xe4), QColor(0xff, 0xff, 0xff),
        QColor(0xb4, 0xb8, 0xc8), QColor(0x2a, 0x73, 0xd9), QColor(0x2f, 0x7d, 0xe1),
        QColor(0x6a, 0x4c, 0xf0), QColor(0x1f, 0x66, 0xc8), QColor(0xff, 0xff, 0xff),
    };
    return light ? bright : dark;
}

QPalette kogEarshotPalette(const KogColors &c)
{
    QPalette palette;
    const auto set = [&palette](QPalette::ColorRole role, const QColor &color) {
        palette.setColor(QPalette::All, role, color);
    };
    set(QPalette::Window, c.window);
    set(QPalette::WindowText, c.text);
    set(QPalette::Base, c.base);
    set(QPalette::AlternateBase, c.alternateBase);
    set(QPalette::Button, c.button);
    set(QPalette::ButtonText, c.text);
    set(QPalette::Text, c.text);
    set(QPalette::BrightText, Qt::white);
    set(QPalette::Light, c.light);
    set(QPalette::Midlight, c.button.lighter(115));
    set(QPalette::Mid, c.border);
    set(QPalette::Dark, c.dark);
    set(QPalette::Shadow, Qt::black);
    set(QPalette::Highlight, c.highlight);
    set(QPalette::HighlightedText, Qt::white);
#if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
    set(QPalette::Accent, c.accent);
#endif
    set(QPalette::Link, c.link);
    set(QPalette::LinkVisited, c.accent2);
    set(QPalette::ToolTipBase, c.tooltip);
    set(QPalette::ToolTipText, c.text);
    set(QPalette::PlaceholderText, c.faint);
    for (const auto role : { QPalette::WindowText, QPalette::Text, QPalette::ButtonText }) {
        palette.setColor(QPalette::Disabled, role, c.faint);
    }
    palette.setColor(QPalette::Disabled, QPalette::Highlight, c.border);
    return palette;
}

QString kogAppearanceMode()
{
    const QString mode = KogSettings(QStringLiteral("Appearance"))
                             .value(QStringLiteral("colorScheme"), QStringLiteral("system"))
                             .toString();
    return mode == QStringLiteral("light") || mode == QStringLiteral("dark")
        ? mode : QStringLiteral("system");
}

// The icon theme for named icons (menus, buttons). Qt's own platform themes
// answer "hicolor" outside Plasma, where Breeze came from the KDE platform
// theme, so Kog picks: the desktop's GTK icon theme (hearth writes it), else
// whatever Qt was given, else Breeze, each in its light or dark variant so
// the icons match Kog's light or dark look.
bool kogIconThemeExists(const QString &name)
{
    for (const QString &path : QIcon::themeSearchPaths()) {
        if (QFileInfo::exists(path + QStringLiteral("/") + name + QStringLiteral("/index.theme"))) {
            return true;
        }
    }
    return false;
}

void kogApplyIconTheme(bool light)
{
    static const QString desktopTheme = [] {
        const QString gtk = QSettings(QDir::homePath() + QStringLiteral("/.config/gtk-3.0/settings.ini"),
                                      QSettings::IniFormat)
                                .value(QStringLiteral("Settings/gtk-icon-theme-name"))
                                .toString();
        if (!gtk.isEmpty() && kogIconThemeExists(gtk)) {
            return gtk;
        }
        const QString qt = QIcon::themeName();
        if (!qt.isEmpty() && qt != QStringLiteral("hicolor")) {
            return qt;
        }
        return QStringLiteral("breeze");
    }();
    QString base = desktopTheme;
    for (const auto *suffix : { "-dark", "-Dark", "_dark" }) {
        if (base.endsWith(QLatin1StringView(suffix))) {
            base.chop(qstrlen(suffix));
            break;
        }
    }
    QString theme = base;
    if (!light) {
        for (const auto *suffix : { "-dark", "-Dark" }) {
            if (kogIconThemeExists(base + QLatin1StringView(suffix))) {
                theme = base + QLatin1StringView(suffix);
                break;
            }
        }
    } else if (!kogIconThemeExists(base)) {
        theme = desktopTheme;
    }
    if (QIcon::themeName() != theme) {
        QIcon::setThemeName(theme);
    }
}

void kogApplyEarshotColors(QApplication &application)
{
    // Light or dark as chosen in Preferences, else as the desktop is.
    const QString mode = kogAppearanceMode();
    // Qt before 6.5 cannot tell the desktop's scheme, so it stays dark.
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
    const bool systemLight = QGuiApplication::styleHints()->colorScheme() == Qt::ColorScheme::Light;
#else
    const bool systemLight = false;
#endif
    // KOG_COLORS=system keeps the desktop's own colour scheme.
    if (qEnvironmentVariable("KOG_COLORS") == QStringLiteral("system")) {
        kogApplyIconTheme(systemLight);
        return;
    }
    const bool light = mode == QStringLiteral("system") ? systemLight : mode == QStringLiteral("light");
    kogApplyIconTheme(light);
    application.setPalette(kogEarshotPalette(kogEarshotColors(light)));
}
// Ubuntu, bundled with the QML, as the interface typeface at the desktop's
// size, with Ubuntu Bold for bold text: the font of the Earshot-look demo.
// egui draws its Ubuntu Light heavier than Qt does, so Regular matches the
// demo's weight and brightness here. KOG_FONT=system keeps the desktop's own font.
void kogApplyInterfaceFont()
{
    if (qEnvironmentVariable("KOG_FONT") == QStringLiteral("system")) {
        return;
    }
    QString family;
    for (const auto *weight : { "Regular", "Bold" }) {
        const int id = QFontDatabase::addApplicationFont(
            QStringLiteral(":/qt/qml/org/kog/player/qml/fonts/Ubuntu-%1.ttf")
                .arg(QLatin1StringView(weight)));
        const QStringList families = QFontDatabase::applicationFontFamilies(id);
        if (family.isEmpty() && !families.isEmpty()) {
            family = families.first();
        }
    }
    if (family.isEmpty()) {
        return;
    }
    QFont font = QGuiApplication::font();
    font.setFamilies({ family });
    // Hinting and anti-aliasing come from the desktop's own font settings
    // (fontconfig), as in every other native app: Kog no longer overrides them.
    // A whole pixel size: the desktop's 10pt is 13.33px, and text at a
    // fractional size wobbled in the file tree and playlist list, which use
    // the application font (the playlist's fixed pixel sizes looked fine).
    if (font.pixelSize() <= 0) {
        font.setPixelSize(qMax(1, qRound(font.pointSizeF() * 96.0 / 72.0)));
    }
    QGuiApplication::setFont(font);
}
} // namespace

KogAppearance::KogAppearance(QObject *parent) : QObject(parent) {}

QString KogAppearance::mode() const { return kogAppearanceMode(); }

void KogAppearance::setMode(const QString &mode)
{
    if (mode == kogAppearanceMode()) {
        return;
    }
    KogSettings(QStringLiteral("Appearance")).setValue(QStringLiteral("colorScheme"), mode);
    if (auto *application = qobject_cast<QApplication *>(QCoreApplication::instance())) {
        kogApplyEarshotColors(*application);
    }
    emit modeChanged();
}

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
        sessionId = KogSettings("MainWindow").value("waylandSessionId").toByteArray();
        if (!sessionId.isEmpty()) {
            arguments[1] = sessionOption.data();
            arguments[2] = sessionId.data();
            argumentCount = 3;
        }
    }
#endif
    auto application = std::make_unique<QApplication>(argumentCount, arguments.data());
    // Every Qt Quick text item draws with the font engine's own hinted
    // glyphs, as native Qt widgets do. Qt's default draws scaled outlines that
    // look soft at fractional display scales such as 130%. tests/text_rendering.rs keeps
    // this line and the QML from drifting back.
    QQuickWindow::setTextRenderType(QQuickWindow::NativeTextRendering);
    kogRegisterModernSkinTypes();
    qmlRegisterSingletonType<KogAppearance>("org.kog.native", 1, 0, "KogAppearance",
                                            [](QQmlEngine *, QJSEngine *) -> QObject * {
                                                return new KogAppearance;
                                            });
    application->setOrganizationName(QStringLiteral("Kog"));
    kogApplyEarshotColors(*application);
    kogApplyInterfaceFont();
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
    // Follow the desktop between Earshot's dark and light variants.
    QObject::connect(QGuiApplication::styleHints(), &QStyleHints::colorSchemeChanged,
                     application.get(), [app = application.get()] { kogApplyEarshotColors(*app); });
#endif
#ifdef KOG_WAYLAND_SESSION_RESTORE
    if (application->platformName().startsWith(QStringLiteral("wayland"))
        && !application->sessionId().isEmpty()) {
        KogSettings settings("MainWindow");
        settings.setValue("waylandSessionId", application->sessionId());
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

// The OS cursor position in global screen pixels. QML hover enter/exit
// around transient items is unreliable on Wayland (opening a surface
// delivers spurious leaves), so the tooltip polls this instead.
QString kogDesktopCursorPos()
{
    const QPoint point = QCursor::pos();
    return QString("%1,%2").arg(point.x()).arg(point.y());
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
