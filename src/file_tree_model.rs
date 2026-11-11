#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;

        include!("kog/kog_desktop_integration.h");
        #[cxx_name = "kogFileIconName"]
        fn themed_file_icon_name(path: &QString) -> QString;
    }

    unsafe extern "C++Qt" {
        include!("kog/kog_file_tree_search.h");
        #[qobject]
        type KogFileTreeSearch;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = KogFileTreeSearch]
        #[qml_element]
        #[qproperty(QModelIndex, root_index)]
        #[qproperty(QString, root_path)]
        #[qproperty(QString, parent_path)]
        #[qproperty(bool, can_go_up)]
        type FileTreeModel = super::FileTreeModelRust;

        #[inherit]
        #[cxx_name = "setRootPath"]
        fn set_root_path_super(self: Pin<&mut FileTreeModel>, path: &QString) -> QModelIndex;

        #[inherit]
        #[cxx_name = "refreshTree"]
        fn refresh_tree_super(self: Pin<&mut FileTreeModel>);

        #[inherit]
        #[cxx_name = "setSupportedFormats"]
        fn set_supported_formats(self: Pin<&mut FileTreeModel>, catalog: &QString);

        #[inherit]
        #[cxx_name = "filePath"]
        fn file_path_super(self: &FileTreeModel, index: &QModelIndex) -> QString;

        #[inherit]
        #[cxx_name = "isDir"]
        fn is_directory_super(self: &FileTreeModel, index: &QModelIndex) -> bool;

        #[cxx_override]
        #[cxx_name = "columnCount"]
        fn column_count(self: &FileTreeModel, parent: &QModelIndex) -> i32;

        #[qinvokable]
        fn set_root_url(self: Pin<&mut FileTreeModel>, url: QUrl);

        #[qinvokable]
        fn set_root_path_text(self: Pin<&mut FileTreeModel>, path: QString);

        #[qinvokable]
        fn refresh_tree(self: Pin<&mut FileTreeModel>);

        #[qinvokable]
        fn file_url(self: &FileTreeModel, index: &QModelIndex) -> QUrl;

        #[qinvokable]
        fn is_directory(self: &FileTreeModel, index: &QModelIndex) -> bool;

        #[qinvokable]
        fn is_path_directory(self: &FileTreeModel, path: QString) -> bool;

        #[qinvokable]
        fn path_url(self: &FileTreeModel, path: QString) -> QUrl;

        #[qinvokable]
        fn path_for_index(self: &FileTreeModel, index: &QModelIndex) -> QString;

        #[qinvokable]
        fn display_name(self: &FileTreeModel, path: QString) -> QString;

        #[qinvokable]
        fn icon_name(self: &FileTreeModel, path: QString) -> QString;
    }
}

use std::ffi::{CStr, c_char, c_void};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cxx_qt_lib::{QModelIndex, QString, QUrl};

#[derive(Default)]
pub struct FileTreeModelRust {
    root_index: QModelIndex,
    root_path: QString,
    parent_path: QString,
    can_go_up: bool,
}

impl qobject::FileTreeModel {
    pub fn column_count(&self, _parent: &QModelIndex) -> i32 {
        1
    }

    pub fn set_root_url(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(path) = url.to_local_file() else {
            return;
        };
        self.as_mut().set_tree_root(PathBuf::from(path.to_string()));
    }

    pub fn set_root_path_text(mut self: Pin<&mut Self>, path: QString) {
        self.as_mut().set_tree_root(PathBuf::from(path.to_string()));
    }

    pub fn refresh_tree(mut self: Pin<&mut Self>) {
        self.as_mut().refresh_tree_super();
    }

    pub fn file_url(&self, index: &QModelIndex) -> QUrl {
        QUrl::from_local_file(&self.file_path_super(index))
    }

    pub fn is_directory(&self, index: &QModelIndex) -> bool {
        self.is_directory_super(index)
    }

    pub fn is_path_directory(&self, path: QString) -> bool {
        Path::new(&path.to_string()).is_dir()
    }

    pub fn path_url(&self, path: QString) -> QUrl {
        QUrl::from_local_file(&path)
    }

    pub fn path_for_index(&self, index: &QModelIndex) -> QString {
        self.file_path_super(index)
    }

    pub fn display_name(&self, path: QString) -> QString {
        QString::from(display_tree_name(&path.to_string()))
    }

    /// A playlist row's format icon. Rows call this from the UI thread for
    /// every row that scrolls into view, so it goes by the file's name alone:
    /// asking about the path itself (kogFileIconName checks for a folder)
    /// would touch the music drive, a spinning disk, once per row.
    pub fn icon_name(&self, path: QString) -> QString {
        let path = path.to_string();
        let name = match kog_audio::archive::tree_location(Path::new(&path)) {
            Ok(Some(location)) if location.directory => return QString::from("folder"),
            Ok(Some(location)) => location.entry,
            _ => path,
        };
        let name = Path::new(&name)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or(name);
        let suffix = Path::new(&name)
            .extension()
            .map(|suffix| suffix.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        thread_local! {
            static ICONS: std::cell::RefCell<std::collections::HashMap<String, String>> =
                std::cell::RefCell::default();
        }
        if !suffix.is_empty()
            && let Some(icon) = ICONS.with(|icons| icons.borrow().get(&suffix).cloned())
        {
            return QString::from(icon.as_str());
        }
        // A bare name: the folder check looks in the working directory,
        // never the music drive.
        let icon = qobject::themed_file_icon_name(&QString::from(name.as_str()));
        if !suffix.is_empty() {
            ICONS.with(|icons| icons.borrow_mut().insert(suffix, icon.to_string()));
        }
        icon
    }

    fn set_tree_root(mut self: Pin<&mut Self>, path: PathBuf) {
        let catalog = kog_audio::decoder::DecoderRegistry::default().supported_formats_json();
        self.as_mut().set_supported_formats(&QString::from(catalog));
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if !Path::new(&path).is_dir() {
            return;
        }

        let parent = path
            .parent()
            .filter(|parent| *parent != path)
            .map(Path::to_path_buf);
        let parent_path = parent
            .as_deref()
            .map(|parent| QString::from(parent.to_string_lossy().as_ref()))
            .unwrap_or_default();
        let can_go_up = parent.is_some();
        let path = QString::from(path.to_string_lossy().as_ref());
        let index = self.as_mut().set_root_path_super(&path);
        self.as_mut().set_parent_path(parent_path);
        self.as_mut().set_can_go_up(can_go_up);
        self.as_mut().set_root_path(path);
        self.as_mut().set_root_index(index);
    }
}

fn display_tree_name(path: &str) -> String {
    let path = match kog_audio::archive::tree_location(Path::new(path)) {
        Ok(Some(location)) => location
            .archive
            .join(location.entry)
            .to_string_lossy()
            .into_owned(),
        _ => path.to_owned(),
    };
    Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_tooltip_uses_basename() {
        let archive = Path::new("/music/Sonic + Classics.zip");
        let member = "Sonic The Hedgehog/Genesis/Green Hill Zone.mid";
        let tree_url = kog_audio::archive::member_url(archive, member, false);
        assert_eq!(
            display_tree_name(tree_url.to_str().unwrap()),
            "Green Hill Zone.mid"
        );
        assert_eq!(
            display_tree_name("/music/Sonic + Classics.zip"),
            "Sonic + Classics.zip"
        );
    }
}

const WALK_DIRECTORY: u32 = 1;
const WALK_MATCHED: u32 = 2;
const WALK_ARCHIVE: u32 = 4;

type WalkVisit = extern "C" fn(*mut c_void, *const c_char, usize, u32, u64) -> bool;

/// The tree search's filesystem pass, shared with the server's walker: see
/// `kog_core::file_search`. One entry reported to C++, or none for a progress
/// report, under `visit`'s lock.
///
/// # Safety
/// `root` and `query` are NUL-terminated, `cancel` and `paused` are valid
/// `std::atomic_bool`s (one byte, like `AtomicBool`), and all outlive the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_file_search_walk(
    root: *const c_char,
    query: *const c_char,
    cancel: *const AtomicBool,
    paused: *const AtomicBool,
    context: *mut c_void,
    visit: WalkVisit,
) {
    use kog_core::file_search::{Matcher, walk};
    let (root, query, cancel, paused) = unsafe {
        (
            CStr::from_ptr(root).to_string_lossy(),
            CStr::from_ptr(query).to_string_lossy(),
            &*cancel,
            &*paused,
        )
    };
    let matcher = Matcher::new(&query);
    let scanned = AtomicU64::new(0);
    let reported = Mutex::new((context as usize, Instant::now()));
    walk(PathBuf::from(&*root), (), |directory, (), subfolders| {
        while paused.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(30));
        }
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            return true;
        };
        let mut found: Vec<(PathBuf, u32)> = Vec::new();
        let mut scratch = Vec::new();
        let mut count = 0;
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            if kog_server::media_filter::is_hidden_name(&name) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            count += 1;
            let mut flags = 0;
            if matcher.is_match(name.as_encoded_bytes(), &mut scratch) {
                flags |= WALK_MATCHED;
            }
            if file_type.is_dir() {
                flags |= WALK_DIRECTORY;
                subfolders.push(entry.path(), ());
            } else if kog_audio::archive::is_path(Path::new(&name)) {
                flags |= WALK_ARCHIVE;
            }
            if flags & (WALK_MATCHED | WALK_ARCHIVE) != 0 {
                found.push((entry.path(), flags));
            }
        }
        let scanned = scanned.fetch_add(count, Ordering::Relaxed) + count;
        let mut proceed = true;
        if found.is_empty() {
            if let Ok(mut report) = reported.try_lock() {
                if report.1.elapsed() >= Duration::from_millis(100) {
                    report.1 = Instant::now();
                    proceed = visit(report.0 as *mut c_void, std::ptr::null(), 0, 0, scanned);
                }
            }
            return proceed;
        }
        let report = reported.lock().unwrap_or_else(|error| error.into_inner());
        for (path, flags) in found {
            let bytes = path.as_os_str().as_encoded_bytes();
            proceed = visit(report.0 as *mut c_void, bytes.as_ptr().cast(), bytes.len(), flags, scanned);
            if !proceed {
                break;
            }
        }
        proceed
    });
}
