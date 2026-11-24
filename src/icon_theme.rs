//! Named icons (menus, window buttons, toolbars) come from Kog's own copy of
//! Breeze and Breeze Dark (qml/icons/breeze, scripts/vendor-breeze-icons.py),
//! never from the desktop's icon theme. Main.qml declares
//!     IconTheme { dark: root.baseLuminance < 0.5 }
//! so the icons follow Kog's light or dark look.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!(<QtGui/QIcon>);
        type QIcon;
        #[Self = "QIcon"]
        #[cxx_name = "setThemeSearchPaths"]
        fn set_theme_search_paths(paths: &QStringList);
        #[Self = "QIcon"]
        #[cxx_name = "setFallbackThemeName"]
        fn set_fallback_theme_name(name: &QString);
        #[Self = "QIcon"]
        #[cxx_name = "setThemeName"]
        fn set_theme_name(name: &QString);
    }
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, dark, READ, WRITE = set_dark, NOTIFY)]
        type IconTheme = super::IconThemeRust;
        fn set_dark(self: Pin<&mut IconTheme>, dark: bool);
    }
}

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QList, QString, QStringList};
use std::pin::Pin;

/// Starts unset, so the first `dark` from QML applies either theme.
#[derive(Default)]
pub struct IconThemeRust {
    dark: bool,
    applied: bool,
}

impl qobject::IconTheme {
    pub fn set_dark(mut self: Pin<&mut Self>, dark: bool) {
        if self.applied && self.dark == dark {
            return;
        }
        if !self.applied {
            let mut paths = QList::<QString>::default();
            paths.append(QString::from(":/kog/icons"));
            qobject::QIcon::set_theme_search_paths(&QStringList::from(&paths));
            qobject::QIcon::set_fallback_theme_name(&QString::from("breeze"));
        }
        qobject::QIcon::set_theme_name(&QString::from(if dark { "breeze-dark" } else { "breeze" }));
        let mut this = self.as_mut().rust_mut();
        this.dark = dark;
        this.applied = true;
        self.dark_changed();
    }
}
