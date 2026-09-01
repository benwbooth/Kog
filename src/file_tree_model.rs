#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    unsafe extern "C++Qt" {
        include!(<QtGui/QFileSystemModel>);
        #[qobject]
        type QFileSystemModel;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QFileSystemModel]
        #[qml_element]
        #[qproperty(QModelIndex, root_index)]
        #[qproperty(QString, root_path)]
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
    }
}

use std::path::{Path, PathBuf};
use std::pin::Pin;

use cxx_qt_lib::{QModelIndex, QString, QUrl};

#[derive(Default)]
pub struct FileTreeModelRust {
    root_index: QModelIndex,
    root_path: QString,
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

    fn set_tree_root(mut self: Pin<&mut Self>, path: PathBuf) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if !Path::new(&path).is_dir() {
            return;
        }

        let path = QString::from(path.to_string_lossy().as_ref());
        let index = self.as_mut().set_root_path_super(&path);
        self.as_mut().set_root_path(path);
        self.as_mut().set_root_index(index);
    }
}
