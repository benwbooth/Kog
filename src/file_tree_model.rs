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
        fn icon_name(self: &FileTreeModel, path: QString) -> QString;
    }
}

use std::path::{Path, PathBuf};
use std::pin::Pin;

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

    pub fn icon_name(&self, path: QString) -> QString {
        qobject::themed_file_icon_name(&path)
    }

    fn set_tree_root(mut self: Pin<&mut Self>, path: PathBuf) {
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
