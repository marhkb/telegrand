use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use log::{debug, info};

use crate::config::{APP_ID, PKGDATADIR, PROFILE, VERSION};
use crate::proxy::proxy_handle_dialog::ProxyTypes;
use crate::proxy::proxy_window::ProxyWindow;
use crate::PreferencesWindow;
use crate::Window;

mod imp {
    use super::*;
    use adw::subclass::prelude::AdwApplicationImpl;
    use glib::WeakRef;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default)]
    pub struct Application {
        pub window: OnceCell<WeakRef<Window>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "Application";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self, app: &Self::Type) {
            debug!("GtkApplication<Application>::activate");

            // register
            ProxyTypes::static_type();

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.show();
                window.present();
                return;
            }

            let window = Window::new(app);
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            app.main_window().present();
        }

        fn startup(&self, app: &Self::Type) {
            debug!("GtkApplication<Application>::startup");

            info!("Telegrand ({})", APP_ID);
            info!("Version: {} ({})", VERSION, PROFILE);
            info!("Datadir: {}", PKGDATADIR);

            self.parent_startup(app);

            // Set icons for shell
            gtk::Window::set_default_icon_name(APP_ID);

            app.setup_gactions();
            app.setup_accels();
            app.load_color_scheme();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &Some(APP_ID)),
            ("flags", &gio::ApplicationFlags::empty()),
            (
                "resource-base-path",
                &Some("/com/github/melix99/telegrand/"),
            ),
        ])
        .expect("Application initialization failed...")
    }

    fn main_window(&self) -> Window {
        let self_ = imp::Application::from_instance(self);
        self_.window.get().unwrap().upgrade().unwrap()
    }

    fn setup_gactions(&self) {
        // Quit
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(clone!(@weak self as app => move |_, _| {
            // This is needed to trigger the delete event and saving the window state
            app.main_window().close();
            app.quit();
        }));
        self.add_action(&action_quit);

        // Proxy
        let action_proxy = gio::SimpleAction::new("proxy", None);
        action_proxy.connect_activate(clone!(@weak self as app => move |_, _| {
            app.show_proxy(app.main_window().client_id())
        }));
        self.add_action(&action_proxy);

        // Preferences
        let action_preferences = gio::SimpleAction::new("preferences", None);
        action_preferences.connect_activate(clone!(@weak self as app => move |_, _| {
            app.show_preferences();
        }));
        self.add_action(&action_preferences);

        // About
        let action_about = gio::SimpleAction::new("about", None);
        action_about.connect_activate(clone!(@weak self as app => move |_, _| {
            app.show_about_dialog();
        }));
        self.add_action(&action_about);
    }

    // Sets up keyboard shortcuts
    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<primary>q"]);
    }

    fn load_color_scheme(&self) {
        let style_manager = adw::StyleManager::default();
        if let Some(style_manager) = style_manager {
            let settings = gio::Settings::new(APP_ID);

            match settings.string("color-scheme").as_ref() {
                "light" => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
                "dark" => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
                _ => style_manager.set_color_scheme(adw::ColorScheme::PreferLight),
            }
        }
    }

    fn show_proxy(&self, client_id: i32) {
        let proxy = ProxyWindow::new(client_id);
        proxy.set_transient_for(Some(&self.main_window()));
        proxy.set_modal(true);
        proxy.present();
    }

    fn show_preferences(&self) {
        let preferences = PreferencesWindow::new();
        preferences.set_transient_for(Some(&self.main_window()));
        preferences.present();
    }

    fn show_about_dialog(&self) {
        let dialog = gtk::AboutDialogBuilder::new()
            .program_name("Telegrand")
            .logo_icon_name(APP_ID)
            .license_type(gtk::License::Gpl30)
            .website("https://github.com/melix99/telegrand/")
            .version(VERSION)
            .transient_for(&self.main_window())
            .modal(true)
            .authors(vec!["Marco Melorio".into(), "Marcus Behrendt".into()])
            .artists(vec![
                "Marco Melorio".into(),
                "Mateus Santos".into(),
                "noëlle".into(),
            ])
            .build();

        dialog.show();
    }
}
