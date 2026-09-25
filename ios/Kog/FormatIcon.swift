import SwiftUI

struct FormatIcon: View {
    let track: Track

    var body: some View {
        Image(iconName)
            .resizable().interpolation(.high).scaledToFit()
            .frame(width: 23, height: 23)
            .accessibilityHidden(true)
    }

    private var iconName: String {
        let name = track.entry.isEmpty ? (track.name.isEmpty ? track.path : track.name) : track.entry
        let ext = URL(fileURLWithPath: name).pathExtension.lowercased()
        let icon: String
        switch ext {
        case "gbs": icon = "gameboy"
        case "nsf", "nsfe": icon = "nes"
        case "spc", "snsf", "minisnsf": icon = "snes"
        case "gsf", "minigsf": icon = "gba"
        case "2sf", "mini2sf", "ncsf", "minincsf": icon = "ds"
        case "psf", "minipsf": icon = "psx"
        case "psf2", "minipsf2": icon = "ps2"
        case "ssf", "minissf", "dsf", "minidsf": icon = "saturn"
        case "usf", "miniusf": icon = "n64"
        case "qsf", "miniqsf": icon = "arcade"
        case "kss": icon = "msx"
        case "hes": icon = "pcengine"
        case "ay": icon = "spectrum"
        case "sap": icon = "atari"
        case "sid": icon = "c64"
        case "hvl", "ahx": icon = "amiga"
        case "vgm", "vgz", "gym", "s98", "dro", "sfm": icon = "chip"
        case "mptm", "mod", "s3m", "xm", "it", "667", "669", "amf", "ams", "c67", "cba",
             "dbm", "digi", "dmf", "dsm", "dsym", "dtm", "etx", "far", "fc", "fc13", "fc14",
             "fmt", "fst", "ftm", "imf", "ims", "ice", "j2b", "m15", "mdl", "med", "mms",
             "mt2", "mtm", "nst", "okt", "plm", "psm", "pt36", "ptm", "puma", "rtm",
             "sfx", "sfx2", "smod", "st26", "stk", "stm", "stx", "stp", "symmod", "tcb",
             "gmc", "gtk", "gt2", "ult", "unic", "wow", "gdm", "mo3", "oxm", "umx",
             "xpk", "ppm", "mmcmp", "org", "jxs": icon = "tracker"
        case "kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf", "hmi",
             "hmp", "hmq", "mus", "xmi": icon = "midi"
        case "aac", "adts", "aif", "aifc", "aiff", "alac", "caf", "flac", "m4a", "m4b",
             "mka", "mkv", "mp1", "mp2", "mp3", "mp4", "oga", "ogg", "ogv", "opus", "wav",
             "wave", "webm", "wma", "asf", "tak", "m4r", "m2a", "mpa", "ape", "ac3",
             "dts", "dtshd", "tta", "vqf", "vqe", "vql", "ra", "rm", "rmj", "weba",
             "dsdiff", "dff", "wsd", "wv", "wvp", "mpc", "shn", "iff", "apl": icon = "audio"
        case "zip", "rar", "7z", "rsn", "vgm7z", "gz", "mdz", "mdr", "s3z", "xmz",
             "itz", "mptmz": icon = "archive"
        case "m3u", "m3u8", "pls": icon = "playlist"
        case "cue": icon = "cue"
        default: icon = "paper"
        }
        return "kog_format_\(icon)"
    }
}
