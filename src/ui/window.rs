use adw::subclass::prelude::*;
use gettextrs::gettext;
use glib::clone;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::CompositeTemplate;

use crate::config;
use crate::model;
use crate::strings;
use crate::types::ChatId;
use crate::types::ClientId;
use crate::ui;
use crate::utils;
use crate::Application;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/window.ui")]
    pub(crate) struct Window {
        pub(super) settings: utils::PaperPlaneSettings,
        #[template_child]
        pub(super) client_manager_view: TemplateChild<ui::ClientManagerView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "PaplWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            obj.client_manager_view()
                .client_manager()
                .connect_update_notification_group(clone!(
                    #[weak]
                    obj,
                    move |_, notification_group, session| {
                        obj.handle_notifications(notification_group, session);
                    }
                ));

            // Devel profile
            if config::PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            // Load latest window state
            obj.load_window_size();
        }
    }

    impl WidgetImpl for Window {}

    impl WindowImpl for Window {
        // Save window state on delete event
        fn close_request(&self) -> glib::Propagation {
            if let Err(err) = self.obj().save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            // Pass close request on to the parent
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}

    #[gtk::template_callbacks]
    impl Window {
        #[template_callback]
        fn on_key_pressed(
            &self,
            key: gdk::Key,
            _: u32,
            modifier: gdk::ModifierType,
            _: &gtk::EventControllerKey,
        ) -> glib::Propagation {
            if key == gdk::Key::w && modifier == gdk::ModifierType::CONTROL_MASK {
                self.obj().close();
            }

            glib::Propagation::Proceed
        }

        #[template_callback]
        fn on_notify_is_active(&self) {
            self.obj().client_manager_view().set_active_client_online();
        }
    }
}

glib::wrapper! {
    pub(crate) struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow;
}

impl Window {
    pub(crate) fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    pub(crate) fn select_chat(&self, client_id: ClientId, chat_id: ChatId) {
        self.client_manager_view().select_chat(client_id, chat_id);
    }

    pub(crate) fn client_manager_view(&self) -> &ui::ClientManagerView {
        &self.imp().client_manager_view
    }

    fn handle_notifications(
        &self,
        notification_group: model::BoxedUpdateNotificationGroup,
        session: &model::ClientStateSession,
    ) {
        let app = self.application().unwrap();

        let chat = session.chat(notification_group.0.chat_id);

        for notification in notification_group.0.added_notifications {
            let notification_id = notification.id;

            let notification = match notification.r#type {
                tdlib::enums::NotificationType::NewMessage(data) => {
                    let message = model::Message::new(&chat, data.message);
                    let mut body = strings::message_content(&message);

                    // Add the sender's name to the body if the chat is a group
                    if matches!(chat.chat_type(), model::ChatType::BasicGroup(_))
                        || matches!(chat.chat_type(), model::ChatType::Supergroup(s) if !s.is_channel())
                    {
                        let sender_name = strings::message_sender(&message.sender(), true);
                        body.insert_str(0, &(sender_name + ": "));
                    }

                    let notification = gio::Notification::new(&chat.title());
                    notification.set_body(Some(&body));

                    Some(notification)
                }
                tdlib::enums::NotificationType::NewCall(_) => {
                    let body = gettext("Incoming call");
                    let notification = gio::Notification::new(&chat.title());
                    notification.set_body(Some(&body));

                    Some(notification)
                }
                _ => None,
            };

            if let Some(notification) = notification {
                notification.set_default_action_and_target_value(
                    "app.select-chat",
                    Some(&(session.client_().id(), chat.id()).to_variant()),
                );

                if let Some(avatar) = chat.avatar() {
                    let avatar_file = &avatar.0;
                    if avatar_file.local.is_downloading_completed {
                        if let Ok(texture) = gdk::Texture::from_filename(&avatar_file.local.path) {
                            notification.set_icon(&texture);
                        }
                        app.send_notification(Some(&notification_id.to_string()), &notification);
                    } else {
                        app.send_notification(Some(&notification_id.to_string()), &notification);

                        let file_id = avatar_file.id;
                        utils::spawn(clone!(
                            #[weak(rename_to = _obj)]
                            self,
                            #[weak]
                            session,
                            #[weak]
                            app,
                            async move {
                                match session.download_file(file_id).await {
                                    Ok(file) => {
                                        let texture =
                                            gdk::Texture::from_filename(file.local.path).unwrap();
                                        notification.set_icon(&texture);

                                        app.send_notification(
                                            Some(&notification_id.to_string()),
                                            &notification,
                                        );
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to download an avatar: {e:?}");
                                    }
                                }
                            }
                        ));
                    }
                }
            }
        }

        for notification_id in notification_group.0.removed_notification_ids {
            app.withdraw_notification(&notification_id.to_string());
        }
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let imp = self.imp();

        let (width, height) = self.default_size();
        imp.settings.set_int("window-width", width)?;
        imp.settings.set_int("window-height", height)?;

        imp.settings
            .set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let imp = self.imp();

        let width = imp.settings.int("window-width");
        let height = imp.settings.int("window-height");
        self.set_default_size(width, height);

        let is_maximized = imp.settings.boolean("is-maximized");
        if is_maximized {
            self.maximize();
        }
    }
}
