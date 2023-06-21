use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use glib::clone;
use gtk::gio;
use gtk::glib;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;

use crate::config::APP_ID;
use crate::utils::spawn;
use crate::Session;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/preferences-window.ui")]
    pub(crate) struct PreferencesWindow {
        pub(super) settings: OnceCell<gio::Settings>,
        pub(super) session: OnceCell<Session>,
        #[template_child]
        pub(super) theme_variants_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub(super) follow_system_colors_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) dark_theme_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) cache_size_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "PreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesWindow;

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

    impl ObjectImpl for PreferencesWindow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<Session>("session")
                    .construct_only()
                    .build()]
            });
            PROPERTIES.as_ref()
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

            let settings = self.settings.get_or_init(|| gio::Settings::new(APP_ID));

            // If the system supports color schemes, load the 'Follow system colors'
            // switch state, otherwise make that switch insensitive
            let style_manager = adw::StyleManager::default();
            if style_manager.system_supports_color_schemes() {
                let follow_system_colors = settings.string("color-scheme") == "default";
                self.follow_system_colors_switch
                    .set_active(follow_system_colors);
            } else {
                self.follow_system_colors_switch.set_sensitive(false);
            }

            let theme_name = settings.string("theme-name");

            let session = self.session.get();

            let preview = crate::components::ThemePreview::new(session);
            let toggle_button = gtk::ToggleButton::builder()
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .child(&preview)
                .active(
                    settings
                        .default_value("theme-name")
                        .unwrap()
                        .get::<String>()
                        .unwrap()
                        == theme_name,
                )
                .build();

            toggle_button.connect_clicked(clone!(@weak settings => move |_| {
                settings.reset("theme-name");
            }));

            let child = gtk::FlowBoxChild::builder()
                .child(&toggle_button)
                .focusable(false)
                .build();

            self.theme_variants_box.append(&child);

            for chat_theme in &*session.unwrap().chat_themes() {
                let preview =
                    crate::components::ThemePreview::from_chat_theme(chat_theme.clone(), session);

                let name = chat_theme.name.clone();

                let button = gtk::ToggleButton::builder()
                    .group(&toggle_button)
                    .halign(gtk::Align::Center)
                    .valign(gtk::Align::Center)
                    .child(&preview)
                    .active(theme_name == name)
                    .build();

                button.connect_clicked(clone!(@weak settings => move |_| {
                    settings.set_string("theme-name", &name).unwrap();
                }));

                let child = gtk::FlowBoxChild::builder()
                    .child(&button)
                    .focusable(false)
                    .build();

                self.theme_variants_box.append(&child);
            }

            obj.setup_bindings();

            spawn(clone!(@weak obj => async move {
                obj.calculate_cache_size().await;
            }));
        }
    }

    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {}
    impl PreferencesWindowImpl for PreferencesWindow {}
}

glib::wrapper! {
    pub(crate) struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window, adw::PreferencesWindow;
}

impl PreferencesWindow {
    pub(crate) fn new(parent_window: Option<&gtk::Window>, session: &Session) -> Self {
        glib::Object::builder()
            .property("transient-for", parent_window)
            .property("session", session)
            .build()
    }

    fn setup_bindings(&self) {
        let imp = self.imp();

        // 'Follow system colors' switch state handling
        imp.follow_system_colors_switch
            .connect_active_notify(|switch| {
                let style_manager = adw::StyleManager::default();
                let settings = gio::Settings::new(APP_ID);
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
        imp.dark_theme_switch.connect_active_notify(
            clone!(@weak follow_system_colors_switch => move |switch| {
                if !follow_system_colors_switch.is_active() {
                    let style_manager = adw::StyleManager::default();
                    let settings = gio::Settings::new(APP_ID);
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
            }),
        );

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
        let client_id = self.session().client_id();
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
        let client_id = self.session().client_id();
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

    pub(crate) fn session(&self) -> &Session {
        self.imp().session.get().unwrap()
    }
}
