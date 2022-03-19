use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use tdlib::enums::{MessageSender as TdMessageSender, Update};
use tdlib::types::Message as TdMessage;

use crate::session::chat::BoxedMessageContent;
use crate::session::{Chat, Session, User};

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "MessageSender")]
pub enum MessageSender {
    User(User),
    Chat(Chat),
}

impl MessageSender {
    pub fn from_td_object(sender: &TdMessageSender, session: &Session) -> Self {
        match sender {
            TdMessageSender::User(data) => {
                let user = session.user_list().get(data.user_id);
                MessageSender::User(user)
            }
            TdMessageSender::Chat(data) => {
                let chat = session.chat_list().get(data.chat_id);
                MessageSender::Chat(chat)
            }
        }
    }

    pub fn id(&self) -> i64 {
        match self {
            Self::User(user) => user.id(),
            Self::Chat(chat) => chat.id(),
        }
    }

    pub fn as_user(&self) -> Option<&User> {
        match self {
            MessageSender::User(user) => Some(user),
            _ => None,
        }
    }
}

mod imp {
    use super::*;
    use glib::WeakRef;
    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default)]
    pub struct Message {
        pub id: Cell<i64>,
        pub sender: OnceCell<MessageSender>,
        pub is_outgoing: Cell<bool>,
        pub date: Cell<i32>,
        pub content: RefCell<Option<BoxedMessageContent>>,
        pub chat: WeakRef<Chat>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Message {
        const NAME: &'static str = "ChatMessage";
        type Type = super::Message;
    }

    impl ObjectImpl for Message {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecInt64::new(
                        "id",
                        "Id",
                        "The id of this message",
                        std::i64::MIN,
                        std::i64::MAX,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecBoxed::new(
                        "sender",
                        "Sender",
                        "The sender of this message",
                        MessageSender::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "is-outgoing",
                        "Is Outgoing",
                        "Whether this message is outgoing or not",
                        false,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecInt::new(
                        "date",
                        "Date",
                        "The point in time when this message was sent",
                        std::i32::MIN,
                        std::i32::MAX,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecBoxed::new(
                        "content",
                        "Content",
                        "The content of this message",
                        BoxedMessageContent::static_type(),
                        glib::ParamFlags::READWRITE
                            | glib::ParamFlags::CONSTRUCT
                            | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecObject::new(
                        "chat",
                        "Chat",
                        "The chat relative to this message",
                        Chat::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "id" => self.id.set(value.get().unwrap()),
                "sender" => self.sender.set(value.get().unwrap()).unwrap(),
                "is-outgoing" => self.is_outgoing.set(value.get().unwrap()),
                "date" => self.date.set(value.get().unwrap()),
                "content" => obj.set_content(value.get().unwrap()),
                "chat" => self.chat.set(Some(&value.get().unwrap())),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "id" => obj.id().to_value(),
                "is-outgoing" => obj.is_outgoing().to_value(),
                "date" => obj.date().to_value(),
                "content" => obj.content().to_value(),
                "chat" => obj.chat().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct Message(ObjectSubclass<imp::Message>);
}

impl Message {
    pub fn new(message: TdMessage, chat: &Chat) -> Self {
        let content = BoxedMessageContent(message.content);

        glib::Object::new(&[
            ("id", &message.id),
            (
                "sender",
                &MessageSender::from_td_object(&message.sender_id, &chat.session()),
            ),
            ("is-outgoing", &message.is_outgoing),
            ("date", &message.date),
            ("content", &content),
            ("chat", chat),
        ])
        .expect("Failed to create Message")
    }

    pub fn handle_update(&self, update: Update) {
        if let Update::MessageContent(data) = update {
            let new_content = BoxedMessageContent(data.new_content);
            self.set_content(new_content);
        }
    }

    pub fn id(&self) -> i64 {
        self.imp().id.get()
    }

    pub fn sender(&self) -> &MessageSender {
        self.imp().sender.get().unwrap()
    }

    pub fn is_outgoing(&self) -> bool {
        self.imp().is_outgoing.get()
    }

    pub fn date(&self) -> i32 {
        self.imp().date.get()
    }

    pub fn content(&self) -> BoxedMessageContent {
        self.imp().content.borrow().as_ref().unwrap().to_owned()
    }

    pub fn set_content(&self, content: BoxedMessageContent) {
        if self.imp().content.borrow().as_ref() == Some(&content) {
            return;
        }
        self.imp().content.replace(Some(content));
        self.notify("content");
    }

    pub fn connect_content_notify<F: Fn(&Self, &glib::ParamSpec) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_notify_local(Some("content"), f)
    }

    pub fn chat(&self) -> Chat {
        self.imp().chat.upgrade().unwrap()
    }

    pub fn sender_name_expression(&self) -> gtk::Expression {
        match self.sender() {
            MessageSender::User(user) => {
                let user_expression = gtk::ConstantExpression::new(user);
                User::full_name_expression(&user_expression)
            }
            MessageSender::Chat(chat) => gtk::ConstantExpression::new(chat)
                .chain_property::<Chat>("title")
                .upcast(),
        }
    }
}
