use cxx_qt_lib::QString;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[namespace = "kog"]
        type AppController = super::AppControllerRust;
    }
}

pub struct AppControllerRust {
    status: QString,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            status: QString::from("CXX-Qt application shell ready"),
        }
    }
}
