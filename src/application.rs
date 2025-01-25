use std::cell::OnceCell;

use adw::prelude::AdwDialogExt;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use glib::clone;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use crate::config;
use crate::ui;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub(crate) struct Application {
        pub(super) window: OnceCell<glib::WeakRef<ui::Window>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "PaplApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self) {
            log::debug!("GtkApplication<Application>::activate");

            let obj = self.obj();

            if let Some(window) = self.window.get() {
                window.upgrade().unwrap().present();
                return;
            }

            let window = ui::Window::new(&obj);
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            obj.main_window().present();
        }

        fn startup(&self) {
            log::debug!("GtkApplication<Application>::startup");

            log::info!("Paper Plane ({})", config::APP_ID);
            log::info!("Version: {} ({})", config::VERSION, config::PROFILE);
            log::info!("Datadir: {}", config::PKGDATADIR);

            self.parent_startup();

            let obj = self.obj();

            // Set icons for shell
            gtk::Window::set_default_icon_name(config::APP_ID);

            obj.setup_gactions();
            obj.setup_accels();
            obj.load_color_scheme();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub(crate) struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub(crate) fn new() -> Self {
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("resource-base-path", "/app/drey/paper-plane/")
            .build()
    }

    fn main_window(&self) -> ui::Window {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    fn setup_gactions(&self) {
        // Quit
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(clone!(
            #[weak(rename_to = app)]
            self,
            move |_, _| {
                // This is needed to trigger the delete event and saving the window state
                app.main_window().close();
                app.quit();
            }
        ));
        self.add_action(&action_quit);

        // About
        let action_about = gio::SimpleAction::new("about", None);
        action_about.connect_activate(clone!(
            #[weak(rename_to = app)]
            self,
            move |_, _| {
                app.show_about_dialog();
            }
        ));
        self.add_action(&action_about);

        // Select chat
        let action_select_chat =
            gio::SimpleAction::new("select-chat", Some(glib::VariantTy::new("(ix)").unwrap()));
        action_select_chat.connect_activate(clone!(
            #[weak(rename_to = app)]
            self,
            move |_, data| {
                let (client_id, chat_id) = data.unwrap().get().unwrap();
                app.main_window().select_chat(client_id, chat_id);
            }
        ));
        self.add_action(&action_select_chat);

        // New login on production server
        let action_new_login_production_server =
            gio::SimpleAction::new("new-login-production-server", None);
        action_new_login_production_server.connect_activate(clone!(
            #[weak(rename_to = app)]
            self,
            move |_, _| {
                app.main_window()
                    .client_manager_view()
                    .add_new_client(false);
            }
        ));
        self.add_action(&action_new_login_production_server);

        // New login on test server
        let action_new_login_test_server = gio::SimpleAction::new("new-login-test-server", None);
        action_new_login_test_server.connect_activate(clone!(
            #[weak(rename_to = app)]
            self,
            move |_, _| {
                app.main_window().client_manager_view().add_new_client(true);
            }
        ));
        self.add_action(&action_new_login_test_server);
    }

    // Sets up keyboard shortcuts
    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<primary>q"]);
    }

    fn load_color_scheme(&self) {
        let style_manager = adw::StyleManager::default();
        let settings = gio::Settings::new(config::APP_ID);
        match settings.string("color-scheme").as_ref() {
            "light" => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
            "dark" => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
            _ => style_manager.set_color_scheme(adw::ColorScheme::PreferLight),
        }
    }

    fn show_about_dialog(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("Paper Plane")
            .application_icon(config::APP_ID)
            .version(config::VERSION)
            .website("https://github.com/paper-plane-developers/paper-plane")
            .issue_url("https://github.com/paper-plane-developers/paper-plane/issues")
            .support_url("https://t.me/paperplanechat")
            .developer_name(gettext("Paper Plane developers"))
            .copyright("© 2021–2023 Marco Melorio")
            .license_type(gtk::License::Gpl30)
            .developers([
                "Karol Lademan https://github.com/karl0d",
                "Marco Melorio (orig. author) https://github.com/melix99",
                "Marcus Behrendt https://github.com/marhkb",
                "Yuri Izmer https://github.com/yuraiz",
            ])
            .designers([
                "Marco Melorio https://github.com/melix99",
                "Yuri Izmer https://github.com/yuraiz",
            ])
            .artists([
                "Mateus Santos https://github.com/swyknox",
                "noëlle https://github.com/jannuary",
            ])
            .build();

        about.add_acknowledgement_section(
            Some(&gettext("Sponsors")),
            &["Alisson Lauffer", "Jordan Maris"],
        );
        about.add_legal_section(
            "Maps",
            Some(&gettext(
                "<span size=\"small\">Map data by \
                <a href=\"https://www.openstreetmap.org\">OpenStreetMap</a> \
                and contributors</span>",
            )),
            gtk::License::Custom,
            Some(&gettext(
                "OpenStreetMap® is open data, licensed under the \
                <a href=\"https://opendatacommons.org/licenses/odbl\">\
                Open Data Commons Open Database License </a> (ODbL) by the \
                <a href=\"https://osmfoundation.org\">OpenStreetMap Foundation</a> (OSMF).",
            )),
        );

        about.present(Some(&self.main_window()));
    }
}
