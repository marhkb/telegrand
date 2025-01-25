use std::cell::OnceCell;
use std::sync::OnceLock;

use gettextrs::gettext;
use glib::clone;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/message_row/indicators.ui")]
    pub(crate) struct MessageIndicators {
        pub(super) message: glib::WeakRef<glib::Object>,
        pub(super) message_signal_group: OnceCell<glib::SignalGroup>,
        pub(super) interaction_info_signal_group: OnceCell<glib::SignalGroup>,
        pub(super) chat_signal_group: OnceCell<glib::SignalGroup>,
        #[template_child]
        pub(super) reply_count_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) reply_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) message_info_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) sending_state_icon: TemplateChild<gtk::Image>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageIndicators {
        const NAME: &'static str = "PaplMessageIndicators";
        type Type = super::MessageIndicators;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("messageindicators");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageIndicators {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![glib::ParamSpecObject::builder::<glib::Object>("message")
                    .explicit_notify()
                    .build()]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "message" => obj.set_message(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "message" => obj.message().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().create_signal_groups();
        }

        fn dispose(&self) {
            let mut child = self.obj().first_child();
            while let Some(child_) = child {
                child = child_.next_sibling();
                child_.unparent();
            }
        }
    }

    impl WidgetImpl for MessageIndicators {}
}

glib::wrapper! {
    pub(crate) struct MessageIndicators(ObjectSubclass<imp::MessageIndicators>)
        @extends gtk::Widget;
}

impl MessageIndicators {
    fn create_signal_groups(&self) {
        let imp = self.imp();

        let message_signal_group = glib::SignalGroup::new::<model::Message>();
        message_signal_group.connect_notify_local(
            Some("is-edited"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_message_info();
                }
            ),
        );
        imp.message_signal_group.set(message_signal_group).unwrap();

        let interaction_info_signal_group =
            glib::SignalGroup::new::<model::MessageInteractionInfo>();
        interaction_info_signal_group.connect_notify_local(
            Some("reply-count"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_reply_count();
                }
            ),
        );
        imp.interaction_info_signal_group
            .set(interaction_info_signal_group)
            .unwrap();

        let chat_signal_group = glib::SignalGroup::new::<model::Chat>();
        chat_signal_group.connect_notify_local(
            Some("last-read-outbox-message-id"),
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, _| {
                    obj.update_sending_state();
                }
            ),
        );
        imp.chat_signal_group.set(chat_signal_group).unwrap();
    }

    pub(crate) fn message(&self) -> Option<glib::Object> {
        self.imp().message.upgrade()
    }

    pub(crate) fn set_message(&self, message: &glib::Object) {
        if self.message().as_ref() == Some(message) {
            return;
        }

        let imp = self.imp();

        imp.message.set(Some(message));

        let message = message.downcast_ref::<model::Message>();

        imp.message_signal_group.get().unwrap().set_target(message);
        imp.interaction_info_signal_group
            .get()
            .unwrap()
            .set_target(message.map(|message| message.interaction_info()).as_ref());
        imp.chat_signal_group
            .get()
            .unwrap()
            .set_target(message.map(|message| message.chat_()).as_ref());

        self.update_reply_count();
        self.update_sending_state();
        self.update_message_info();

        self.notify("message");
    }

    fn update_reply_count(&self) {
        let imp = self.imp();

        let message = self.message().and_downcast::<model::Message>();

        let is_channel_message = message.as_ref()
            .filter(|message| {
                matches!(message.chat_().chat_type(), model::ChatType::Supergroup(data) if data.is_channel())
            })
            .is_some();

        if is_channel_message {
            imp.reply_count_label.set_label("");
            imp.reply_count_box.set_visible(false);
        } else {
            let reply_count = message
                .as_ref()
                .map(model::Message::interaction_info)
                .map(|interaction_info| interaction_info.reply_count())
                .unwrap_or(0);

            if reply_count > 0 {
                imp.reply_count_label.set_label(&reply_count.to_string());
                imp.reply_count_box.set_visible(true);
            } else {
                imp.reply_count_label.set_label("");
                imp.reply_count_box.set_visible(false);
            }
        }
    }

    fn update_sending_state(&self) {
        let icon_name = self
            .message()
            .and_downcast::<model::Message>()
            .filter(|message| message.is_outgoing())
            .map(|message| match message.sending_state() {
                Some(state) => match state.0 {
                    tdlib::enums::MessageSendingState::Failed(_) => "message-failed-symbolic",
                    tdlib::enums::MessageSendingState::Pending(_) => "message-pending-symbolic",
                },
                None => {
                    let chat = message.chat_();

                    if chat.is_own_chat() || message.id() <= chat.last_read_outbox_message_id() {
                        "message-read-symbolic"
                    } else {
                        "message-unread-left-symbolic"
                    }
                }
            });

        let imp = self.imp();

        if let Some(icon_name) = icon_name {
            imp.sending_state_icon.set_icon_name(Some(icon_name));
            imp.sending_state_icon.set_visible(true);
        } else {
            imp.sending_state_icon.set_icon_name(None);
            imp.sending_state_icon.set_visible(false);
        }
    }

    fn update_message_info(&self) {
        let message = self.message();

        let label = if let Some(message) = message.and_downcast_ref::<model::Message>() {
            let datetime = glib::DateTime::from_unix_utc(message.date() as i64)
                .and_then(|t| t.to_local())
                // Translators: This is a time representation, without seconds.
                // Here you may want to change to a 24-hours representation, based on your locale.
                // You can use this site to learn more: https://www.strfti.me/
                .and_then(|t| t.format(&gettext("%l:%M %p")))
                .unwrap();

            if message.is_edited()
                && !matches!(
                    message.content().0,
                    tdlib::enums::MessageContent::MessageLocation(_)
                )
            {
                format!("{} {}", gettext("edited"), datetime)
            } else {
                datetime.into()
            }
        } else if message
            .and_downcast_ref::<model::SponsoredMessage>()
            .is_some()
        {
            gettext("sponsored")
        } else {
            unreachable!()
        };

        self.imp().message_info_label.set_label(&label);
    }
}
