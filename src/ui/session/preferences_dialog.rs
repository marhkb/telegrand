use std::cell::OnceCell;
use std::sync::OnceLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use glib::clone;
use gtk::gio;
use gtk::glib;
use gtk::CompositeTemplate;

use crate::config;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/preferences_dialog.ui")]
    pub(crate) struct PreferencesDialog {
        pub(super) session: OnceCell<ui::Session>,
        #[template_child]
        pub(super) follow_system_colors_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) dark_theme_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) cache_size_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesDialog {
        const NAME: &'static str = "PaplPreferencesDialog";
        type Type = super::PreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action_async(
                "preferences.clear-cache",
                None,
                |widget, _, _| async move {
                    widget.clear_cache().await;
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesDialog {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![glib::ParamSpecObject::builder::<ui::Session>("session")
                    .construct_only()
                    .build()]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "session" => self.session.set(value.get().unwrap()).unwrap(),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "session" => obj.session().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // If the system supports color schemes, load the 'Follow system colors'
            // switch state, otherwise make that switch insensitive
            let style_manager = adw::StyleManager::default();
            if style_manager.system_supports_color_schemes() {
                let settings = gio::Settings::new(config::APP_ID);
                let follow_system_colors = settings.string("color-scheme") == "default";
                self.follow_system_colors_switch
                    .set_active(follow_system_colors);
            } else {
                self.follow_system_colors_switch.set_sensitive(false);
            }

            obj.setup_bindings();

            utils::spawn(clone!(
                #[weak]
                obj,
                async move {
                    obj.calculate_cache_size().await;
                }
            ));
        }
    }

    impl WidgetImpl for PreferencesDialog {}
    impl AdwDialogImpl for PreferencesDialog {}
    impl PreferencesDialogImpl for PreferencesDialog {}
}

glib::wrapper! {
    pub(crate) struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog;
}

impl From<&ui::Session> for PreferencesDialog {
    fn from(session: &ui::Session) -> Self {
        glib::Object::builder().property("session", session).build()
    }
}

impl PreferencesDialog {
    fn setup_bindings(&self) {
        let imp = self.imp();

        // 'Follow system colors' switch state handling
        imp.follow_system_colors_switch
            .connect_active_notify(|switch| {
                let style_manager = adw::StyleManager::default();
                let settings = gio::Settings::new(config::APP_ID);
                if switch.is_active() {
                    // Prefer light theme unless the system prefers dark colors
                    style_manager.set_color_scheme(adw::ColorScheme::PreferLight);
                    settings.set_string("color-scheme", "default").unwrap();
                } else {
                    // Set default state for the dark theme switch
                    style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
                    settings.set_string("color-scheme", "light").unwrap();
                }
            });

        // 'Dark theme' switch state handling
        let follow_system_colors_switch = &*imp.follow_system_colors_switch;
        imp.dark_theme_switch.connect_active_notify(clone!(
            #[weak]
            follow_system_colors_switch,
            move |switch| {
                if !follow_system_colors_switch.is_active() {
                    let style_manager = adw::StyleManager::default();
                    let settings = gio::Settings::new(config::APP_ID);
                    if switch.is_active() {
                        // Dark mode
                        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
                        settings.set_string("color-scheme", "dark").unwrap();
                    } else {
                        // Light mode
                        style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
                        settings.set_string("color-scheme", "light").unwrap();
                    }
                }
            }
        ));

        // Make the 'Dark theme' switch insensitive if the 'Follow system colors'
        // switch is active
        imp.follow_system_colors_switch
            .bind_property("active", &*imp.dark_theme_switch, "sensitive")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::INVERT_BOOLEAN)
            .build();

        // Have the 'Dark theme' switch state always updated with the dark state
        let style_manager = adw::StyleManager::default();
        style_manager
            .bind_property("dark", &*imp.dark_theme_switch, "active")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    async fn calculate_cache_size(&self) {
        let client_id = self.session().model().unwrap().client_().id();
        match tdlib::functions::get_storage_statistics(0, client_id).await {
            Ok(tdlib::enums::StorageStatistics::StorageStatistics(data)) => {
                let size = glib::format_size(data.size as u64);
                self.imp().cache_size_label.set_label(&size);
            }
            Err(e) => {
                log::warn!("Error getting the storage statistics: {e:?}");
            }
        }
    }

    async fn clear_cache(&self) {
        let client_id = self.session().model().unwrap().client_().id();
        match tdlib::functions::optimize_storage(
            0,
            0,
            0,
            0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            false,
            0,
            client_id,
        )
        .await
        {
            Ok(tdlib::enums::StorageStatistics::StorageStatistics(data)) => {
                let size = glib::format_size(data.size as u64);
                self.imp().cache_size_label.set_label(&size);

                self.add_toast(adw::Toast::new(&gettext("Cache cleared")));
            }
            Err(e) => {
                log::warn!("Error optimizing the storage: {e:?}");
            }
        }
    }

    pub(crate) fn session(&self) -> &ui::Session {
        self.imp().session.get().unwrap()
    }
}
