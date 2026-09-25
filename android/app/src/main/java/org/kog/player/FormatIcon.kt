package org.kog.player

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp

/** Rasterized from the same Qt SVGs embedded by the web frontend. */
@Composable
fun FormatIcon(track: Track, modifier: Modifier = Modifier) {
    Image(painter = painterResource(formatIconRes(track)), contentDescription = null,
        modifier = modifier.size(23.dp))
}

@Composable
fun FolderIcon(modifier: Modifier = Modifier) {
    Image(painter = painterResource(R.drawable.kog_folder), contentDescription = null,
        modifier = modifier.size(23.dp))
}

private fun formatIconRes(track: Track): Int {
    val name = track.entry.ifBlank { track.name.ifBlank { track.path } }
    val extension = name.substringAfterLast('.', "").lowercase()
    return when (extension) {
        "gbs" -> R.drawable.kog_format_gameboy
        "nsf", "nsfe" -> R.drawable.kog_format_nes
        "spc", "snsf", "minisnsf" -> R.drawable.kog_format_snes
        "gsf", "minigsf" -> R.drawable.kog_format_gba
        "2sf", "mini2sf", "ncsf", "minincsf" -> R.drawable.kog_format_ds
        "psf", "minipsf" -> R.drawable.kog_format_psx
        "psf2", "minipsf2" -> R.drawable.kog_format_ps2
        "ssf", "minissf", "dsf", "minidsf" -> R.drawable.kog_format_saturn
        "usf", "miniusf" -> R.drawable.kog_format_n64
        "qsf", "miniqsf" -> R.drawable.kog_format_arcade
        "kss" -> R.drawable.kog_format_msx
        "hes" -> R.drawable.kog_format_pcengine
        "ay" -> R.drawable.kog_format_spectrum
        "sap" -> R.drawable.kog_format_atari
        "sid" -> R.drawable.kog_format_c64
        "hvl", "ahx" -> R.drawable.kog_format_amiga
        "vgm", "vgz", "gym", "s98", "dro", "sfm" -> R.drawable.kog_format_chip
        "mptm", "mod", "s3m", "xm", "it", "667", "669", "amf", "ams", "c67", "cba",
        "dbm", "digi", "dmf", "dsm", "dsym", "dtm", "etx", "far", "fc", "fc13", "fc14",
        "fmt", "fst", "ftm", "imf", "ims", "ice", "j2b", "m15", "mdl", "med", "mms",
        "mt2", "mtm", "nst", "okt", "plm", "psm", "pt36", "ptm", "puma", "rtm",
        "sfx", "sfx2", "smod", "st26", "stk", "stm", "stx", "stp", "symmod", "tcb",
        "gmc", "gtk", "gt2", "ult", "unic", "wow", "gdm", "mo3", "oxm", "umx",
        "xpk", "ppm", "mmcmp", "org", "jxs" -> R.drawable.kog_format_tracker
        "kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf", "hmi",
        "hmp", "hmq", "mus", "xmi" -> R.drawable.kog_format_midi
        "aac", "adts", "aif", "aifc", "aiff", "alac", "caf", "flac", "m4a", "m4b",
        "mka", "mkv", "mp1", "mp2", "mp3", "mp4", "oga", "ogg", "ogv", "opus", "wav",
        "wave", "webm", "wma", "asf", "tak", "m4r", "m2a", "mpa", "ape", "ac3",
        "dts", "dtshd", "tta", "vqf", "vqe", "vql", "ra", "rm", "rmj", "weba",
        "dsdiff", "dff", "wsd", "wv", "wvp", "mpc", "shn", "iff", "apl" -> R.drawable.kog_format_audio
        "zip", "rar", "7z", "rsn", "vgm7z", "gz", "mdz", "mdr", "s3z", "xmz",
        "itz", "mptmz" -> R.drawable.kog_format_archive
        "m3u", "m3u8", "pls" -> R.drawable.kog_format_playlist
        "cue" -> R.drawable.kog_format_cue
        else -> R.drawable.kog_format_paper
    }
}
