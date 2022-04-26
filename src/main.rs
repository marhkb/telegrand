mod application;
#[rustfmt::skip]
#[allow(clippy::all)]
mod config;
mod country_info;
mod country_list;
mod expressions;
mod login;
mod phone_number_input;
mod preferences_window;
mod session;
mod session_manager;
mod take_picture_dialog;
mod utils;
mod window;

use self::application::Application;
use self::login::Login;
use self::preferences_window::PreferencesWindow;
use self::session::Session;
use self::window::Window;

use config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};
use gettextrs::{gettext, LocaleCategory};
use gtk::prelude::{ApplicationExt, ApplicationExtManual, IsA};
use gtk::{gio, glib};
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::str::FromStr;
use temp_dir::TempDir;

pub(crate) static APPLICATION_OPTS: OnceCell<ApplicationOptions> = OnceCell::new();
pub(crate) static TEMP_DIR: OnceCell<PathBuf> = OnceCell::new();

fn main() {
    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name("Telegrand");

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    gst::init().expect("Unable to init gstreamer");
    gst4gtk::plugin_register_static().expect("Failed to register gstgtk4 plugin");

    let app = setup_cli(Application::new());

    // Command line handling
    app.connect_handle_local_options(|_, dict| {
        if dict.contains("version") {
            // Print version ...
            println!("telegrand {}", config::VERSION);
            // ... and exit application.
            1
        } else {
            let log_level = match dict.lookup::<String>("log-level").unwrap() {
                Some(level) => log::Level::from_str(&level).expect("Error on parsing log-level"),
                // Standard log levels if not specified by user
                None => log::Level::Warn,
            };

            let mut application_opts = ApplicationOptions::default();

            // TODO: Change to syslog when tdlib v1.8 is out where messages can be redirected.
            std::env::set_var("RUST_LOG", log_level.as_str());
            pretty_env_logger::init();

            if dict.contains("test-dc") {
                application_opts.test_dc = true;
            }

            APPLICATION_OPTS.set(application_opts).unwrap();

            -1
        }
    });

    // Create temp directory.
    // This value must live during the entire execution of the app.
    let temp_dir = TempDir::with_prefix("telegrand");
    match &temp_dir {
        Ok(temp_dir) => {
            TEMP_DIR.set(temp_dir.path().to_path_buf()).unwrap();
        }
        Err(e) => {
            log::warn!("Error creating temp directory: {:?}", e);
        }
    }

    app.run();
}

/// Global options for the application
#[derive(Debug)]
pub(crate) struct ApplicationOptions {
    pub(crate) data_dir: PathBuf,
    pub(crate) test_dc: bool,
}
impl Default for ApplicationOptions {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(glib::user_data_dir().to_str().unwrap()).join("telegrand"),
            test_dc: Default::default(),
        }
    }
}

fn setup_cli<A: IsA<gio::Application>>(app: A) -> A {
    app.add_main_option(
        "version",
        b'v'.into(),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        &gettext("Prints application version"),
        None,
    );

    app.add_main_option(
        "log-level",
        b'l'.into(),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        &gettext("Specify the minimum log level"),
        Some("error|warn|info|debug|trace"),
    );

    app.add_main_option(
        "test-dc",
        b't'.into(),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        &gettext("Whether to use a test data center on first account"),
        None,
    );

    app
}
