#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("kog/kog_desktop_integration.h");

        type QApplication;

        #[cxx_name = "kogApplicationNew"]
        fn application_new() -> UniquePtr<QApplication>;

        #[cxx_name = "kogApplicationSetName"]
        fn application_set_name(application: Pin<&mut QApplication>, name: &QString);

        #[cxx_name = "kogApplicationExec"]
        fn application_exec(application: Pin<&mut QApplication>) -> i32;

        #[cxx_name = "kogApplyApplicationIcon"]
        fn apply_application_icon();

        #[cxx_name = "kogRestoreMainWindow"]
        fn restore_main_window();

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }
}

pub struct DesktopApplication(cxx::UniquePtr<ffi::QApplication>);

impl DesktopApplication {
    pub fn new() -> Self {
        Self(ffi::application_new())
    }

    pub fn set_application_name(&mut self, name: &cxx_qt_lib::QString) {
        if let Some(application) = self.0.as_mut() {
            ffi::application_set_name(application, name);
        }
    }

    pub fn exec(&mut self) -> i32 {
        self.0
            .as_mut()
            .map(ffi::application_exec)
            .unwrap_or_default()
    }
}

pub fn apply_application_icon() {
    ffi::apply_application_icon();
}

pub fn restore_main_window() {
    ffi::restore_main_window();
}
