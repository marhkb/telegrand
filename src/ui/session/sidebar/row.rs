use std::cell::OnceCell;
use std::sync::OnceLock;

use gettextrs::gettext;
use glib::clone;
use glib::closure;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::expressions;
use crate::model;
use crate::strings;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/sidebar/row.ui")]
    pub(crate) struct Row {
        pub(super) item: glib::WeakRef<model::ChatListItem>,
        pub(super) item_signal_group: OnceCell<glib::SignalGroup>,
        pub(super) chat_signal_group: OnceCell<glib::SignalGroup>,
        pub(super) session_signal_group: OnceCell<glib::SignalGroup>,
        #[template_child]
        pub(super) title_label: TemplateChild<gtk::Inscription>,
        #[template_child]
        pub(super) message_status_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) timestamp_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) subtitle_prefix_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) minithumbnail: TemplateChild<ui::SidebarMiniThumbnail>,
        #[template_child]
        pub(super) subtitle_label: TemplateChild<gtk::Inscription>,
        #[template_child]
        pub(super) status_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) pin_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) unread_mention_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) unread_count_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "PaplSidebarRow";
        type Type = super::Row;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action("sidebar-row.archive", None, move |widget, _, _| {
                widget.toggle_chat_is_archived()
            });
            klass.install_action("sidebar-row.unarchive", None, move |widget, _, _| {
                widget.toggle_chat_is_archived()
            });
            klass.install_action("sidebar-row.pin", None, move |widget, _, _| {
                widget.toggle_chat_is_pinned()
            });
            klass.install_action("sidebar-row.unpin", None, move |widget, _, _| {
                widget.toggle_chat_is_pinned()
            });
            klass.install_action("sidebar-row.mark-as-unread", None, move |widget, _, _| {
                widget.toggle_chat_marked_as_unread()
            });
            klass.install_action("sidebar-row.mark-as-read", None, move |widget, _, _| {
                widget.toggle_chat_marked_as_unread()
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl Row {
        #[template_callback]
        fn on_pressed(&self, _n_press: i32, x: f64, y: f64) {
            self.show_menu(x as i32, y as i32);
        }

        #[template_callback]
        fn on_long_pressed(&self, x: f64, y: f64) {
            self.show_menu(x as i32, y as i32);
        }

        fn show_menu(&self, x: i32, y: i32) {
            let obj = &*self.obj();
            let sidebar = utils::ancestor::<_, ui::Sidebar>(obj);
            let menu = sidebar.row_menu();

            menu.set_pointing_to(Some(&gdk::Rectangle::new(x, y, 0, 0)));
            menu.unparent();
            menu.set_parent(obj);
            menu.popup();
        }
    }

    impl ObjectImpl for Row {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::ChatListItem>("item")
                        .explicit_notify()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "item" => obj.set_item(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "item" => obj.item().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            obj.setup_expressions();
            obj.create_signal_groups();
        }

        fn dispose(&self) {
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for Row {}
}

glib::wrapper! {
    pub(crate) struct Row(ObjectSubclass<imp::Row>)
        @extends gtk::Widget;
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Row {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }

    pub(crate) fn toggle_chat_is_archived(&self) {
        if let Some(item) = self.item() {
            let chat = item.chat_();
            utils::spawn(clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    if let Err(e) = tdlib::functions::add_chat_to_list(
                        chat.id(),
                        match item.chat_list_type().0 {
                            tdlib::enums::ChatList::Main => tdlib::enums::ChatList::Archive,
                            tdlib::enums::ChatList::Archive => tdlib::enums::ChatList::Main,
                            _ => return,
                        },
                        chat.session_().client_().id(),
                    )
                    .await
                    {
                        utils::show_toast(&obj, e.message);
                    }
                }
            ));
        }
    }

    fn toggle_chat_is_pinned(&self) {
        if let Some(item) = self.item() {
            utils::spawn(async move {
                if let Err(e) = item.toggle_is_pinned().await {
                    log::warn!("Error on toggling chat's pinned state: {e:?}");
                }
            });
        }
    }

    fn toggle_chat_marked_as_unread(&self) {
        if let Some(chat) = self.item().map(|i| i.chat_()) {
            utils::spawn(async move {
                if let Err(e) = if chat.unread_count() > 0 || chat.is_marked_as_unread() {
                    chat.mark_as_read().await
                } else {
                    chat.mark_as_unread().await
                } {
                    log::warn!("Error on toggling chat's unread state: {e:?}");
                }
            });
        }
    }

    fn setup_expressions(&self) {
        let imp = self.imp();
        let item_expression = Self::this_expression("item");
        let chat_expression = item_expression.chain_property::<model::ChatListItem>("chat");

        // Chat title
        expressions::chat_display_name(&chat_expression).bind(
            &*imp.title_label,
            "text",
            Some(self),
        );

        // Chat unread count
        chat_expression
            .chain_property::<model::Chat>("unread-count")
            .chain_closure::<String>(closure!(|_: Self, unread_count: i32| {
                if unread_count > 0 {
                    unread_count.to_string()
                } else {
                    String::new()
                }
            }))
            .bind(&*imp.unread_count_label, "label", Some(self));
    }

    fn create_signal_groups(&self) {
        let imp = self.imp();

        let item_signal_group = glib::SignalGroup::new::<model::ChatListItem>();
        item_signal_group.connect_notify_local(
            Some("is-pinned"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_status_stack();
                    obj.update_actions();
                }
            ),
        );
        imp.item_signal_group.set(item_signal_group).unwrap();

        let chat_signal_group = glib::SignalGroup::new::<model::Chat>();
        chat_signal_group.connect_notify_local(
            Some("is-marked-as-unread"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_status_stack();
                    obj.update_actions();
                }
            ),
        );
        chat_signal_group.connect_notify_local(
            Some("unread-count"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_status_stack();
                    obj.update_actions();
                }
            ),
        );
        chat_signal_group.connect_notify_local(
            Some("unread-mention-count"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_status_stack();
                }
            ),
        );
        chat_signal_group.connect_notify_local(
            Some("last-read-outbox-message-id"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_message_status_icon();
                }
            ),
        );
        chat_signal_group.connect_notify_local(
            Some("last-message"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_message_status_icon();
                    obj.update_timestamp();
                    obj.update_subtitle_prefix_label();
                    obj.update_minithumbnail();
                    obj.update_subtitle_label();
                }
            ),
        );
        chat_signal_group.connect_notify_local(
            Some("draft-message"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_timestamp();
                    obj.update_subtitle_prefix_label();
                    obj.update_minithumbnail();
                    obj.update_subtitle_label();
                }
            ),
        );
        chat_signal_group.connect_notify_local(
            Some("actions"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_subtitle_prefix_label();
                    obj.update_minithumbnail();
                    obj.update_subtitle_label();
                }
            ),
        );
        chat_signal_group.connect_notify_local(
            Some("notification-settings"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_unread_count_style();
                }
            ),
        );
        imp.chat_signal_group.set(chat_signal_group).unwrap();

        let session_signal_group = glib::SignalGroup::new::<model::ClientStateSession>();
        session_signal_group.connect_notify_local(
            Some("private-chats-notification-settings"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_unread_count_style();
                }
            ),
        );
        session_signal_group.connect_notify_local(
            Some("group-chats-notification-settings"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_unread_count_style();
                }
            ),
        );
        session_signal_group.connect_notify_local(
            Some("channel-chats-notification-settings"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_unread_count_style();
                }
            ),
        );
        imp.session_signal_group.set(session_signal_group).unwrap();
    }

    pub(crate) fn item(&self) -> Option<model::ChatListItem> {
        self.imp().item.upgrade()
    }

    pub(crate) fn set_item(&self, item: Option<&model::ChatListItem>) {
        if self.item().as_ref() == item {
            return;
        }

        let imp = self.imp();

        imp.item_signal_group.get().unwrap().set_target(item);
        imp.chat_signal_group
            .get()
            .unwrap()
            .set_target(item.map(|i| i.chat_()).as_ref());
        imp.session_signal_group
            .get()
            .unwrap()
            .set_target(item.map(|i| i.chat_().session_()).as_ref());

        imp.item.set(item);

        self.update_message_status_icon();
        self.update_timestamp();
        self.update_subtitle_prefix_label();
        self.update_minithumbnail();
        self.update_subtitle_label();
        self.update_status_stack();
        self.update_unread_count_style();
        self.update_actions();

        self.notify("item");
    }

    fn update_message_status_icon(&self) {
        if let Some(item) = self.item() {
            let imp = self.imp();
            let chat = item.chat_();
            let icon = &imp.message_status_icon;

            if chat.is_own_chat() {
                icon.set_visible(false);
            } else if let Some(message) = chat.last_message().filter(model::Message::is_outgoing) {
                let (icon_name, css_class) = match message.sending_state() {
                    Some(state) => match state.0 {
                        tdlib::enums::MessageSendingState::Failed(_) => {
                            ("message-failed-symbolic", "error")
                        }
                        tdlib::enums::MessageSendingState::Pending(_) => {
                            ("message-pending-symbolic", "dim-label")
                        }
                    },
                    None => (
                        if message.id() == chat.last_read_outbox_message_id() {
                            "message-read-symbolic"
                        } else {
                            "message-unread-right-symbolic"
                        },
                        "accent",
                    ),
                };

                icon.set_icon_name(Some(icon_name));
                icon.set_css_classes(&[css_class]);
                icon.set_visible(true);
            } else {
                icon.set_visible(false);
            }
        }
    }

    fn update_timestamp(&self) {
        if let Some(item) = self.item() {
            let imp = self.imp();
            let chat = item.chat_();
            let label = &imp.timestamp_label;

            let date = chat
                .draft_message()
                .map(|m| m.0.date)
                .or_else(|| chat.last_message().map(|m| m.date()));

            if let Some(date) = date {
                label.set_label(&timestamp_text(date as i64));
                label.set_visible(true);
            } else {
                label.set_visible(false);
            }
        }
    }

    fn update_subtitle_prefix_label(&self) {
        if let Some(item) = self.item() {
            let imp = self.imp();
            let chat = item.chat_();
            let label = &imp.subtitle_prefix_label;

            if chat.actions().last().is_some() {
                label.set_visible(false);
            } else if chat.draft_message().is_some() {
                label.set_label(&gettext("Draft:"));
                label.add_css_class("error");
                label.set_visible(true);
            } else if let Some(sender) = chat.last_message().and_then(sender_label) {
                label.set_label(&format!("{sender}:"));
                label.remove_css_class("error");
                label.set_visible(true);
            } else {
                label.set_visible(false);
            }
        }
    }

    fn update_minithumbnail(&self) {
        if let Some(item) = self.item() {
            let imp = self.imp();
            let chat = item.chat_();
            let minithumbnail = &imp.minithumbnail;

            if chat.actions().n_items() > 0 || chat.draft_message().is_some() {
                minithumbnail.set_visible(false);
            } else if let Some(texture) = chat.last_message().and_then(message_thumbnail_texture) {
                minithumbnail.set_paintable(Some(texture.upcast()));
                minithumbnail.set_visible(true);
            } else {
                minithumbnail.set_visible(false);
            }
        }
    }

    fn update_subtitle_label(&self) {
        if let Some(item) = self.item() {
            let imp = self.imp();
            let chat = item.chat_();
            let label = &imp.subtitle_label;

            if let Some(actions) = chat.actions().last().map(|a| strings::chat_action(&a)) {
                label.set_text(Some(&actions));
                label.remove_css_class("dim-label");
                label.add_css_class("accent");
            } else if let Some(draft_message) =
                chat.draft_message().map(|d| draft_message_text(d.0))
            {
                label.set_text(Some(&draft_message));
                label.add_css_class("dim-label");
                label.remove_css_class("accent");
            } else if let Some(last_message) =
                chat.last_message().map(|m| strings::message_content(&m))
            {
                label.set_text(Some(&last_message));
                label.add_css_class("dim-label");
                label.remove_css_class("accent");
            } else {
                label.set_text(None);
            }
        }
    }

    fn update_status_stack(&self) {
        if let Some(item) = self.item() {
            let imp = self.imp();
            let chat = item.chat_();
            let stack = &imp.status_stack;

            if chat.unread_mention_count() > 0 {
                stack.set_visible_child(&*imp.unread_mention_label);
            } else if chat.unread_count() > 0 || chat.is_marked_as_unread() {
                stack.set_visible_child(&*imp.unread_count_label);
            } else if item.is_pinned() {
                stack.set_visible_child(&*imp.pin_icon);
            } else {
                stack.set_visible_child_name("empty");
            }
        }
    }

    fn update_unread_count_style(&self) {
        if let Some(item) = self.item() {
            let imp = self.imp();
            let chat = item.chat_();
            let label = &imp.unread_count_label;

            let notification_settings = chat.notification_settings();
            let scope_notification_settings = match chat.chat_type() {
                model::ChatType::Private(_) | model::ChatType::Secret(_) => {
                    chat.session_().private_chats_notification_settings()
                }
                model::ChatType::Supergroup(supergroup) if supergroup.is_channel() => {
                    chat.session_().channel_chats_notification_settings()
                }
                _ => chat.session_().group_chats_notification_settings(),
            };

            let css_class = if notification_settings.0.use_default_mute_for {
                if scope_notification_settings.0.mute_for > 0
                    || notification_settings.0.mute_for > 0
                {
                    "unread-count-muted"
                } else {
                    "unread-count-unmuted"
                }
            } else if notification_settings.0.mute_for > 0 {
                "unread-count-muted"
            } else {
                "unread-count-unmuted"
            };

            label.set_css_classes(&["unread-count", css_class]);
        }
    }

    fn update_actions(&self) {
        if let Some(item) = self.item() {
            let chat = item.chat_();

            if chat.is_own_chat() {
                self.update_archive_actions(false, false);
            } else {
                match item.chat_list_type().0 {
                    tdlib::enums::ChatList::Main => self.update_archive_actions(true, false),
                    tdlib::enums::ChatList::Archive => self.update_archive_actions(false, true),
                    _ => self.update_archive_actions(false, false),
                }
            }

            self.update_pin_actions(!item.is_pinned(), item.is_pinned());

            if chat.unread_count() > 0 {
                self.update_mark_as_unread_actions(false, true);
            } else {
                self.update_mark_as_unread_actions(
                    !chat.is_marked_as_unread(),
                    chat.is_marked_as_unread(),
                );
            }
        } else {
            self.update_archive_actions(false, false);
            self.update_pin_actions(false, false);
            self.update_mark_as_unread_actions(false, false);
        }
    }

    fn update_archive_actions(&self, archive: bool, unarchive: bool) {
        self.action_set_enabled("sidebar-row.archive", archive);
        self.action_set_enabled("sidebar-row.unarchive", unarchive);
    }

    fn update_pin_actions(&self, pin: bool, unpin: bool) {
        self.action_set_enabled("sidebar-row.pin", pin);
        self.action_set_enabled("sidebar-row.unpin", unpin);
    }

    fn update_mark_as_unread_actions(&self, unread: bool, read: bool) {
        self.action_set_enabled("sidebar-row.mark-as-unread", unread);
        self.action_set_enabled("sidebar-row.mark-as-read", read);
    }
}

fn sender_label(message: model::Message) -> Option<String> {
    use tdlib::enums::MessageContent::*;

    // Just use a sender label for specific messages.
    match message.content().0 {
        MessageText(_) | MessageLocation(_) | MessageSticker(_) | MessagePhoto(_)
        | MessageAudio(_) | MessageAnimation(_) | MessageVideo(_) | MessageDocument(_)
        | MessageVoiceNote(_) | MessageCall(_) | MessageVenue(_) => {}
        _ => return None,
    }

    if message.chat_().is_own_chat() {
        if message.is_outgoing() {
            None
        } else {
            message
                .forward_info()
                .map(|forward_info| forward_info.origin())
                .map(|forward_origin| match forward_origin {
                    model::MessageForwardOrigin::User(user) => {
                        strings::user_display_name(&user, false)
                    }
                    model::MessageForwardOrigin::Chat { chat, .. }
                    | model::MessageForwardOrigin::Channel { chat, .. } => chat.title(),
                    model::MessageForwardOrigin::HiddenUser { sender_name }
                    | model::MessageForwardOrigin::MessageImport { sender_name } => {
                        sender_name.clone()
                    }
                })
        }
    } else {
        let show_sender = match message.chat_().chat_type() {
            model::ChatType::BasicGroup(_) => true,
            model::ChatType::Supergroup(supergroup) => !supergroup.is_channel(),
            model::ChatType::Private(_) | model::ChatType::Secret(_) => message.is_outgoing(),
        };
        if show_sender {
            Some(if message.is_outgoing() {
                gettext("You")
            } else {
                strings::message_sender(&message.sender(), false)
            })
        } else {
            None
        }
    }
}

fn message_thumbnail_texture(message: model::Message) -> Option<gdk::Texture> {
    use tdlib::enums::MessageContent::*;

    match message.content().0 {
        MessageAnimation(data) => data.animation.minithumbnail,
        MessageAudio(data) => data.audio.album_cover_minithumbnail,
        MessageChatChangePhoto(data) => data.photo.minithumbnail,
        MessageDocument(data) => data.document.minithumbnail,
        MessagePhoto(data) => data.photo.minithumbnail,
        MessageVideo(data) => data.video.minithumbnail,
        _ => None,
    }
    .map(|thumbnail| {
        gdk::Texture::from_bytes(&glib::Bytes::from_owned(glib::base64_decode(
            &thumbnail.data,
        )))
        .unwrap()
    })
}

fn draft_message_text(message: tdlib::types::DraftMessage) -> String {
    match message.input_message_text {
        tdlib::enums::InputMessageContent::InputMessageText(data) => data.text.text,
        other => {
            log::warn!("Unexpected draft message type: {other:?}");
            String::new()
        }
    }
}

fn timestamp_text(date: i64) -> glib::GString {
    let datetime_now = glib::DateTime::now_local().unwrap();
    let datetime = glib::DateTime::from_unix_utc(date)
        .and_then(|t| t.to_local())
        .unwrap();

    let difference = datetime_now.difference(&datetime);
    let hours_difference = difference.as_hours();
    let days_difference = difference.as_days();

    if hours_difference <= 16 {
        datetime.format(&gettext("%l:%M %p"))
    } else if days_difference < 6 {
        // Show the day of the week
        datetime.format("%a")
    } else if days_difference < 364 {
        // Show the day and the month
        datetime.format("%d %b")
    } else {
        // Show the entire date
        datetime.format("%x")
    }
    .unwrap()
}
