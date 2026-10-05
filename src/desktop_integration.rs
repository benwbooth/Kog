#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("kog/kog_settings_bridge.h");
        #[cxx_name = "kogConfigureSettings"]
        fn configure_settings(port: fn(key: &QString, value: &QString, write: bool) -> QString);
        include!("kog/kog_tree_archive_bridge.h");
        #[cxx_name = "kogConfigureArchiveDecoder"]
        fn configure_archive_decoder(decoder: fn(bytes: &[u8]) -> String);
        include!("kog/kog_desktop_integration.h");
        include!("kog/kog_single_instance.h");

        #[cxx_name = "kogSingleInstanceStart"]
        fn single_instance_start() -> QString;

        type QApplication;

        #[cxx_name = "kogApplicationNew"]
        fn application_new() -> UniquePtr<QApplication>;

        #[cxx_name = "kogApplicationSetName"]
        fn application_set_name(application: Pin<&mut QApplication>, name: &QString);

        #[cxx_name = "kogApplicationSetVersion"]
        fn application_set_version(application: Pin<&mut QApplication>, version: &QString);

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

pub enum GuiInstance {
    Primary,
    Raised,
}

pub fn claim_gui_instance() -> Result<GuiInstance, String> {
    match ffi::single_instance_start().to_string().as_str() {
        "primary" => Ok(GuiInstance::Primary),
        "raised" => Ok(GuiInstance::Raised),
        error => Err(error.to_owned()),
    }
}

pub struct DesktopApplication(cxx::UniquePtr<ffi::QApplication>);

impl DesktopApplication {
    pub fn new() -> Self {
        ffi::configure_settings(settings_port);
        ffi::configure_archive_decoder(kog_core::text_encoding::decode);
        Self(ffi::application_new())
    }

    pub fn set_application_name(&mut self, name: &cxx_qt_lib::QString) {
        if let Some(application) = self.0.as_mut() {
            ffi::application_set_name(application, name);
        }
        if let Some(application) = self.0.as_mut() {
            ffi::application_set_version(
                application,
                &cxx_qt_lib::QString::from(env!("CARGO_PKG_VERSION")),
            );
        }
    }

    pub fn exec(&mut self) -> i32 {
        self.0
            .as_mut()
            .map(ffi::application_exec)
            .unwrap_or_default()
    }
}

fn settings_port(
    key: &cxx_qt_lib::QString,
    value: &cxx_qt_lib::QString,
    write: bool,
) -> cxx_qt_lib::QString {
    let result = (|| {
        let db = kog_core::db::LibraryDb::open()?;
        let key = key.to_string();
        let value = value.to_string();
        if write {
            db.put_state("preferences", &key, &value)?;
            return Ok(value);
        }
        if let Some(saved) = db.load_state("preferences", &key)? {
            return Ok(saved.value);
        }
        if value.is_empty() {
            return Ok(String::new());
        }
        db.import_state("preferences", &key, &value)
            .map(|saved| saved.value)
    })();
    match result {
        Ok(value) => cxx_qt_lib::QString::from(value),
        Err(error) => {
            eprintln!("Kog settings: {error}");
            cxx_qt_lib::QString::default()
        }
    }
}

pub fn apply_application_icon() {
    ffi::apply_application_icon();
}

pub fn restore_main_window() {
    ffi::restore_main_window();
}
