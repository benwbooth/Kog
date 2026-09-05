#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
        include!("kog/kog_skin_network.h");
        #[cxx_name = "kogFetchSkinUrl"]
        fn fetch_skin_url(url: &QString, max_bytes: u32) -> Result<QByteArray>;
        #[cxx_name = "kogValidateSkinImage"]
        fn validate_skin_image(path: &QString, min_width: u32, min_height: u32) -> bool;
        #[cxx_name = "kogSkinTextColors"]
        fn skin_text_colors(path: &QString) -> QString;
        #[cxx_name = "kogValidateModernSkin"]
        fn validate_modern_skin(path: &QString) -> bool;
        #[cxx_name = "kogValidateModernImage"]
        fn validate_modern_image(path: &QString) -> bool;
    }
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, catalog_json)]
        #[qproperty(QString, installed_json)]
        #[qproperty(QString, active_json)]
        #[qproperty(QString, status)]
        #[qproperty(bool, busy)]
        #[qproperty(i32, total)]
        #[qproperty(bool, modern)]
        type SkinLibrary = super::SkinLibraryRust;
        #[qinvokable]
        fn search(self: Pin<&mut SkinLibrary>, query: QString, page: i32);
        #[qinvokable]
        fn install(self: Pin<&mut SkinLibrary>, identifier: QString);
        #[qinvokable]
        fn import_file(self: Pin<&mut SkinLibrary>);
        #[qinvokable]
        fn apply(self: Pin<&mut SkinLibrary>, index: i32);
        #[qinvokable]
        fn poll(self: Pin<&mut SkinLibrary>);
    }
}

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    pin::Pin,
    sync::mpsc::{self, Receiver},
};

enum Outcome {
    Catalog(Value, i32),
    Installed(Value),
    Cancelled,
}
pub struct SkinLibraryRust {
    catalog_json: QString,
    installed_json: QString,
    active_json: QString,
    status: QString,
    busy: bool,
    total: i32,
    modern: bool,
    worker: Option<Receiver<Result<Outcome, String>>>,
    installed: Vec<Value>,
}

fn data_dir() -> Result<PathBuf, String> {
    directories::ProjectDirs::from("org", "Kog", "Kog")
        .map(|p| p.data_local_dir().join("skins"))
        .ok_or_else(|| "Cannot locate Kog's skin library directory".into())
}

fn installed_skins() -> Vec<Value> {
    let Ok(dir) = data_dir() else {
        return Vec::new();
    };
    let mut skins: Vec<Value> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| fs::read(entry.path().join("skin.json")).ok())
        .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
        .collect();
    skins.sort_by_key(|skin| skin["title"].as_str().unwrap_or("").to_lowercase());
    skins
}

impl Default for SkinLibraryRust {
    fn default() -> Self {
        let installed = installed_skins();
        Self {
            installed_json: QString::from(json!(installed).to_string()),
            installed,
            catalog_json: QString::from("[]"),
            active_json: QString::from("{}"),
            status: QString::from("Browse Internet Archive, or import a .wsz / .wal / .zip skin."),
            busy: false,
            total: 0,
            modern: false,
            worker: None,
        }
    }
}

fn fetch(url: &str, limit: u32) -> Result<Vec<u8>, String> {
    qobject::fetch_skin_url(&QString::from(url), limit)
        .map(|bytes| bytes.as_slice().to_vec())
        .map_err(|e| e.to_string())
}

fn valid_identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
}

fn search_catalog(query: &str, page: i32, modern: bool) -> Result<Outcome, String> {
    // Treat input as words, not Archive's query language.
    let words: Vec<_> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .take(12)
        .collect();
    let mut q = if modern {
        "collection:winampskinsmodern AND mediatype:software"
    } else {
        "collection:winampskins AND NOT collection:winampskinsmodern AND mediatype:software"
    }
    .to_owned();
    for word in words {
        q.push_str(&format!(" AND title:({word}*)"));
    }
    let mut url = url::Url::parse("https://archive.org/advancedsearch.php").unwrap();
    url.query_pairs_mut()
        .append_pair("q", &q)
        .append_pair("fl[]", "identifier")
        .append_pair("fl[]", "title")
        .append_pair("rows", "24")
        .append_pair("page", &page.max(1).to_string())
        .append_pair("sort[]", "downloads desc")
        .append_pair("output", "json");
    let result: Value = serde_json::from_slice(&fetch(url.as_str(), 2 * 1024 * 1024)?)
        .map_err(|e| e.to_string())?;
    let response = &result["response"];
    let docs = response["docs"]
        .as_array()
        .ok_or("Invalid Archive search response")?;
    let items: Vec<_> = docs
        .iter()
        .filter_map(|doc| {
            let id = doc["identifier"].as_str()?;
            if !valid_identifier(id) {
                return None;
            }
            Some(json!({"id": id, "title": doc["title"].as_str().unwrap_or(id)}))
        })
        .collect();
    Ok(Outcome::Catalog(
        json!(items),
        response["numFound"]
            .as_i64()
            .unwrap_or(0)
            .clamp(0, i32::MAX as i64) as i32,
    ))
}

fn install_remote(id: &str) -> Result<Outcome, String> {
    if !valid_identifier(id) {
        return Err("Invalid Internet Archive identifier".into());
    }
    let meta: Value = serde_json::from_slice(&fetch(
        &format!("https://archive.org/metadata/{id}"),
        2 * 1024 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    let files = meta["files"]
        .as_array()
        .ok_or("This item has no downloadable files")?;
    let mut candidates: Vec<&str> = files
        .iter()
        .filter_map(|file| {
            let name = file["name"].as_str()?;
            let lower = name.to_lowercase();
            (lower.ends_with(".wsz") || lower.ends_with(".wal") || lower.ends_with(".zip"))
                .then_some(name)
        })
        .collect();
    candidates.sort_by_key(|name| (!name.to_lowercase().ends_with(".wsz"), *name));
    // Collection bundles must not silently install an arbitrary skin.
    let name = match candidates.as_slice() {
        [name] => *name,
        [] => return Err("No .wsz, .wal or .zip skin in this Archive item.".into()),
        _ => return Err("This item contains multiple skin archives. Open its source page and import the skin you want.".into()),
    };
    let mut address = url::Url::parse(&format!("https://archive.org/download/{id}/")).unwrap();
    address
        .path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(name);
    let bytes = fetch(address.as_str(), 32 * 1024 * 1024)?;
    let mut archive = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    archive.write_all(&bytes).map_err(|e| e.to_string())?;
    let title = meta["metadata"]["title"].as_str().unwrap_or(id);
    install_archive(
        archive.path(),
        title,
        &format!("https://archive.org/details/{id}"),
        &meta["metadata"],
    )
    .map(Outcome::Installed)
}

fn install_archive(
    path: &Path,
    title: &str,
    source: &str,
    metadata: &Value,
) -> Result<Value, String> {
    install_archive_in(path, title, source, metadata, &data_dir()?)
}

fn install_archive_in(
    path: &Path,
    title: &str,
    source: &str,
    metadata: &Value,
    root: &Path,
) -> Result<Value, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Skin archive exceeds the 32 MiB limit".into());
    }
    // Validate the same immutable bytes that will be installed, even if the
    // user replaces the original file while an import is running.
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Skin archive exceeds the 32 MiB limit".into());
    }
    let mut snapshot = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    snapshot.write_all(&bytes).map_err(|e| e.to_string())?;
    let archive = crate::archive::ExtractedArchive::open_skin(snapshot.path())?;
    if !archive.warnings.is_empty() {
        return Err("Skin contains unsafe or ambiguous archive entries".into());
    }
    let modern_roots: Vec<_> = archive
        .entries
        .iter()
        .filter(|entry| {
            entry
                .path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("skin.xml"))
        })
        .collect();
    if !modern_roots.is_empty() {
        if modern_roots.len() != 1 {
            return Err("Multiple skin.xml files: import one modern skin at a time".into());
        }
        if !qobject::validate_modern_skin(&QString::from(
            modern_roots[0].path.to_string_lossy().as_ref(),
        )) {
            return Err(
                "Invalid modern skin.xml (expected WinampAbstractionLayer or WasabiXML)".into(),
            );
        }
        if !bytes.starts_with(b"PK\x03\x04") {
            return Err("Modern skins must use ZIP .wal archives".into());
        }
        let mut names = std::collections::HashSet::new();
        if archive
            .entries
            .iter()
            .any(|entry| !names.insert(entry.name.replace('\\', "/").to_lowercase()))
        {
            return Err("Modern skin contains ambiguous case-insensitive filenames".into());
        }
        for entry in &archive.entries {
            let ext = entry
                .path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if ["bmp", "png", "jpg", "jpeg", "gif", "webp"].contains(&ext.as_str())
                && !qobject::validate_modern_image(&QString::from(
                    entry.path.to_string_lossy().as_ref(),
                ))
            {
                return Err(format!(
                    "Invalid or oversized modern skin image: {}",
                    entry.name
                ));
            }
        }
        if archive.entries.iter().any(|entry| {
            entry.path.extension().is_some_and(|ext| {
                ["dll", "exe", "com", "scr", "so", "dylib"]
                    .contains(&ext.to_string_lossy().to_ascii_lowercase().as_str())
            })
        }) {
            return Err("Modern skins with native executable plugins are not supported".into());
        }
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let staging = tempfile::Builder::new()
            .prefix("modern-")
            .tempdir_in(root)
            .map_err(|e| e.to_string())?;
        let archive_path = staging.path().join("skin.wal");
        fs::write(&archive_path, bytes).map_err(|e| e.to_string())?;
        let skin = json!({"kind":"modern", "title":title, "source":source,
            "creator":metadata["creator"], "license":metadata["licenseurl"],
            "archivePath":archive_path.to_string_lossy()});
        fs::write(
            staging.path().join("skin.json"),
            serde_json::to_vec_pretty(&skin).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        let _ = staging.keep();
        return Ok(skin);
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let staging = tempfile::Builder::new()
        .prefix("classic-")
        .tempdir_in(root)
        .map_err(|e| e.to_string())?;
    let mut assets = serde_json::Map::new();
    let mut text_colors = "#000000,#71f5b0".to_owned();
    for (name, width, height, required) in [
        ("main", 275, 116, true),
        ("cbuttons", 136, 36, true),
        ("titlebar", 302, 29, false),
        ("numbers", 90, 13, false),
        ("playpaus", 27, 9, false),
        ("posbar", 278, 10, false),
        ("volume", 68, 433, false),
        ("shufrep", 92, 73, false),
        ("text", 150, 12, false),
        ("pledit", 280, 110, false),
    ] {
        let target_name = format!("{name}.bmp");
        let matching: Vec<_> = archive
            .entries
            .iter()
            .filter(|e| {
                e.path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&target_name))
            })
            .collect();
        if matching.len() > 1 {
            return Err(format!(
                "Multiple {target_name} files: import one skin at a time"
            ));
        }
        let Some(entry) = matching.first() else {
            if required {
                return Err(format!(
                    "Not a supported classic skin: missing {target_name}. Modern .wal skins need a different engine."
                ));
            }
            continue;
        };
        if !qobject::validate_skin_image(
            &QString::from(entry.path.to_string_lossy().as_ref()),
            width,
            height,
        ) {
            return Err(format!("Invalid or oversized {target_name} bitmap"));
        }
        let destination = staging.path().join(&target_name);
        fs::copy(&entry.path, &destination).map_err(|e| e.to_string())?;
        if name == "text" {
            text_colors =
                qobject::skin_text_colors(&QString::from(destination.to_string_lossy().as_ref()))
                    .to_string();
        }
        assets.insert(
            name.into(),
            json!(
                url::Url::from_file_path(destination)
                    .map_err(|_| "Invalid skin path")?
                    .as_str()
            ),
        );
    }
    let mut playlist_colors = serde_json::Map::new();
    for entry in &archive.entries {
        if entry
            .path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("pledit.txt"))
        {
            let bytes = fs::read(&entry.path).map_err(|e| e.to_string())?;
            if bytes.len() > 16 * 1024 {
                return Err("Oversized pledit.txt".into());
            }
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim().to_ascii_lowercase();
                    let value = value.trim().trim_start_matches('#');
                    if ["normal", "current", "normalbg", "selectedbg"].contains(&key.as_str())
                        && value.len() == 6
                        && value.bytes().all(|c| c.is_ascii_hexdigit())
                    {
                        playlist_colors.insert(key, json!(format!("#{value}")));
                    }
                }
            }
        }
    }
    let skin = json!({"kind": "classic", "title": title, "source": source, "creator": metadata["creator"],
        "license": metadata["licenseurl"], "assets": assets, "textColors": text_colors,
        "playlistColors": playlist_colors});
    fs::write(
        staging.path().join("skin.json"),
        serde_json::to_vec_pretty(&skin).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    // Only validated passive bitmaps and attribution survive. Scripts, DLLs,
    // executables and arbitrary archive contents are never installed or run.
    let _ = staging.keep();
    Ok(skin)
}

impl qobject::SkinLibrary {
    fn start(
        mut self: Pin<&mut Self>,
        message: &str,
        work: impl FnOnce() -> Result<Outcome, String> + Send + 'static,
    ) {
        if self.rust().busy {
            return;
        }
        let (send, receive) = mpsc::channel();
        self.as_mut().rust_mut().worker = Some(receive);
        self.as_mut().set_busy(true);
        self.as_mut().set_status(QString::from(message));
        std::thread::spawn(move || {
            let _ = send.send(work());
        });
    }
    pub fn search(self: Pin<&mut Self>, query: QString, page: i32) {
        let query = query.to_string();
        let modern = self.rust().modern;
        self.start("Searching Internet Archive…", move || {
            search_catalog(&query, page, modern)
        });
    }
    pub fn install(self: Pin<&mut Self>, identifier: QString) {
        let id = identifier.to_string();
        self.start("Downloading and validating skin…", move || {
            install_remote(&id)
        });
    }
    pub fn import_file(self: Pin<&mut Self>) {
        self.start("Choose a skin…", || {
            let Some(path) = rfd::FileDialog::new()
                .set_title("Import Winamp skin")
                .add_filter("Winamp skin", &["wsz", "wal", "zip"])
                .pick_file()
            else {
                return Ok(Outcome::Cancelled);
            };
            let title = path.file_stem().unwrap_or_default().to_string_lossy();
            install_archive(&path, &title, "", &Value::Null).map(Outcome::Installed)
        });
    }
    pub fn apply(mut self: Pin<&mut Self>, index: i32) {
        if let Some(skin) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().installed.get(i))
            .cloned()
        {
            self.as_mut()
                .set_active_json(QString::from(skin.to_string()));
        }
    }
    pub fn poll(mut self: Pin<&mut Self>) {
        let result = match self.rust().worker.as_ref().map(|r| r.try_recv()) {
            Some(Ok(result)) => result,
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                Err("Skin worker stopped unexpectedly".into())
            }
            _ => return,
        };
        self.as_mut().rust_mut().worker = None;
        self.as_mut().set_busy(false);
        match result {
            Ok(Outcome::Catalog(items, total)) => {
                self.as_mut()
                    .set_catalog_json(QString::from(items.to_string()));
                self.as_mut().set_total(total);
                self.as_mut()
                    .set_status(QString::from(format!("{total} skins on Internet Archive")));
            }
            Ok(Outcome::Installed(skin)) => {
                self.as_mut().rust_mut().installed = installed_skins();
                let json = json!(self.rust().installed).to_string();
                self.as_mut().set_installed_json(QString::from(json));
                self.as_mut()
                    .set_active_json(QString::from(skin.to_string()));
                self.as_mut().set_status(QString::from(
                    "Skin installed. Use “Open player” to listen.",
                ));
            }
            Ok(Outcome::Cancelled) => self.as_mut().set_status(QString::from("Import cancelled.")),
            Err(error) => self.as_mut().set_status(QString::from(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_identifiers_cannot_inject_urls() {
        assert!(valid_identifier("winampskins_DigiTool_v1.2"));
        for bad in ["", "../foo", "a?x=y", "a#b", "a/b", "https://evil.example"] {
            assert!(!valid_identifier(bad));
        }
    }

    fn bitmap() -> Vec<u8> {
        let (width, height) = (275_u32, 116_u32);
        let stride = (width * 3 + 3) & !3;
        let length = 54 + stride * height;
        let mut bytes = vec![0_u8; length as usize];
        bytes[0..2].copy_from_slice(b"BM");
        bytes[2..6].copy_from_slice(&length.to_le_bytes());
        bytes[10..14].copy_from_slice(&54_u32.to_le_bytes());
        bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
        bytes[18..22].copy_from_slice(&width.to_le_bytes());
        bytes[22..26].copy_from_slice(&height.to_le_bytes());
        bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
        bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
        bytes
    }

    #[test]
    fn imports_case_insensitive_bitmaps_but_no_active_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skin.wsz");
        let bmp = bitmap();
        crate::archive::tests::write_stored_zip(
            &path,
            &[
                ("Skin/MAIN.BMP", &bmp),
                ("Skin/CBUTTONS.BMP", &bmp),
                ("skin.maki", b"ignored script"),
                ("vis.dll", b"ignored binary"),
            ],
        );
        let skin = install_archive_in(
            &path,
            "Test",
            "https://archive.org/details/test",
            &json!({"creator":"Artist"}),
            &dir.path().join("installed"),
        )
        .unwrap();
        assert_eq!(skin["creator"], "Artist");
        let main = url::Url::parse(skin["assets"]["main"].as_str().unwrap())
            .unwrap()
            .to_file_path()
            .unwrap();
        assert!(main.is_file());
        let folder = main.parent().unwrap();
        assert!(folder.join("cbuttons.bmp").is_file());
        assert!(!folder.join("skin.maki").exists());
        assert!(!folder.join("vis.dll").exists());
        assert_eq!(fs::read_dir(folder).unwrap().count(), 3);
    }

    #[test]
    fn rejects_traversal_duplicate_sheets_and_modern_skins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skin.zip");
        let bmp = bitmap();
        for entries in [
            vec![
                ("main.bmp", bmp.as_slice()),
                ("cbuttons.bmp", bmp.as_slice()),
                ("../escape", b"bad".as_slice()),
            ],
            vec![
                ("main.bmp", bmp.as_slice()),
                ("MAIN.BMP", bmp.as_slice()),
                ("cbuttons.bmp", bmp.as_slice()),
            ],
            vec![("skin.xml", b"modern".as_slice())],
            vec![
                ("main.bmp", b"not a bitmap".as_slice()),
                ("cbuttons.bmp", bmp.as_slice()),
            ],
        ] {
            crate::archive::tests::write_stored_zip(&path, &entries);
            assert!(
                install_archive_in(
                    &path,
                    "Test",
                    "",
                    &Value::Null,
                    &dir.path().join("installed")
                )
                .is_err()
            );
        }
    }

    #[test]
    fn imports_modern_archive_and_preserves_only_validated_zip_and_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("modern.wal");
        crate::archive::tests::write_stored_zip(&path, &[
            ("Example/skin.xml", b"<WinampAbstractionLayer version=\"1.0\"><skininfo><name>Example</name></skininfo></WinampAbstractionLayer>"),
            ("Example/readme.txt", b"Example attribution"),
        ]);
        let skin = install_archive_in(
            &path,
            "Example",
            "",
            &Value::Null,
            &dir.path().join("installed"),
        )
        .unwrap();
        assert_eq!(skin["kind"], "modern");
        let stored = PathBuf::from(skin["archivePath"].as_str().unwrap());
        assert_eq!(fs::read(&stored).unwrap(), fs::read(&path).unwrap());
        assert_eq!(fs::read_dir(stored.parent().unwrap()).unwrap().count(), 2);
        assert!(skin["assets"].is_null());
    }

    #[test]
    fn imports_reference_modern_skins() {
        let root = tempfile::tempdir().unwrap();
        for name in ["MMD3.wal", "WinampModern566.wal", "CornerAmp_Redux.wal"] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("native/webamp/packages/webamp-modern/assets/skins")
                .join(name);
            let skin = install_archive_in(&path, name, "", &Value::Null, root.path())
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(skin["kind"], "modern");
        }
    }

    #[test]
    fn imports_playlist_artwork_and_valid_colors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("classic.wsz");
        let bmp = bitmap();
        crate::archive::tests::write_stored_zip(
            &path,
            &[
                ("main.bmp", &bmp),
                ("cbuttons.bmp", &bmp),
                (
                    "PLEDIT.TXT",
                    b"[Text]\nNormal=#123456\nCurrent=ABCDEF\nSelectedBG=not-a-color\nFont=bad\n",
                ),
            ],
        );
        let skin = install_archive_in(
            &path,
            "Example",
            "",
            &Value::Null,
            &dir.path().join("installed"),
        )
        .unwrap();
        assert_eq!(skin["playlistColors"]["normal"], "#123456");
        assert_eq!(skin["playlistColors"]["current"], "#ABCDEF");
        assert!(skin["playlistColors"]["selectedbg"].is_null());
        assert!(skin["playlistColors"]["font"].is_null());
    }

    #[test]
    #[ignore = "requires KOG_TEST_SKIN_ARCHIVE pointing to a locally downloaded real skin"]
    fn imports_real_classic_skin() {
        let path = PathBuf::from(
            std::env::var_os("KOG_TEST_SKIN_ARCHIVE").expect("set KOG_TEST_SKIN_ARCHIVE"),
        );
        let root = tempfile::tempdir().unwrap();
        let skin = install_archive_in(&path, "Real skin", "", &Value::Null, root.path()).unwrap();
        assert!(skin["assets"]["main"].as_str().is_some());
        assert!(skin["assets"]["cbuttons"].as_str().is_some());
    }
}
