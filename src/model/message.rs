use std::cell::Cell;
use std::cell::OnceCell;
use std::cell::RefCell;

use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::expressions;
use crate::model;
use crate::types::MessageSenderId;

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "MessageSender")]
pub(crate) enum MessageSender {
    User(model::User),
    Chat(model::Chat),
}

impl MessageSender {
    pub(crate) fn new(
        session: &model::ClientStateSession,
        sender: &tdlib::enums::MessageSender,
    ) -> Self {
        use tdlib::enums::MessageSender::*;

        match sender {
            User(data) => Self::User(session.user(data.user_id)),
            Chat(data) => Self::Chat(session.chat(data.chat_id)),
        }
    }

    pub(crate) fn as_user(&self) -> Option<&model::User> {
        match self {
            Self::User(user) => Some(user),
            _ => None,
        }
    }

    pub(crate) fn id(&self) -> MessageSenderId {
        match self {
            Self::User(user) => user.id(),
            Self::Chat(chat) => chat.id(),
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Properties, Default)]
    #[properties(wrapper_type = super::Message)]
    pub(crate) struct Message {
        #[property(get, set, construct_only)]
        pub(super) chat: glib::WeakRef<model::Chat>,
        #[property(get, set, construct_only)]
        pub(super) id: OnceCell<i64>,
        #[property(get, set, construct_only)]
        pub(super) sender: OnceCell<MessageSender>,
        #[property(get, set, construct_only)]
        pub(super) is_outgoing: OnceCell<bool>,
        #[property(get, set, construct_only)]
        pub(super) can_be_edited: OnceCell<bool>,
        #[property(get, set, construct_only)]
        pub(super) can_be_deleted_only_for_self: OnceCell<bool>,
        #[property(get, set, construct_only)]
        pub(super) can_be_deleted_for_all_users: OnceCell<bool>,
        #[property(get, set, construct_only)]
        pub(super) sending_state: OnceCell<Option<model::BoxedMessageSendingState>>,
        #[property(get, set, construct_only)]
        pub(super) date: OnceCell<i32>,
        #[property(get, set, construct_only)]
        pub(super) interaction_info: OnceCell<model::MessageInteractionInfo>,
        #[property(get, set, construct_only)]
        pub(super) forward_info: OnceCell<Option<model::MessageForwardInfo>>,
        #[property(get, set, construct_only)]
        pub(super) reply_to: OnceCell<Option<model::BoxedMessageReplyTo>>,
        #[property(get)]
        pub(super) content: RefCell<model::BoxedMessageContent>,
        #[property(get)]
        pub(super) is_edited: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Message {
        const NAME: &'static str = "Message";
        type Type = super::Message;
    }

    impl ObjectImpl for Message {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }
}

glib::wrapper! {
    pub(crate) struct Message(ObjectSubclass<imp::Message>);
}

impl Message {
    pub(crate) fn new(chat: &model::Chat, td_message: tdlib::types::Message) -> Self {
        let obj: Self = glib::Object::builder()
            .property("chat", chat)
            .property("id", td_message.id)
            .property(
                "sender",
                model::MessageSender::new(&chat.session_(), &td_message.sender_id),
            )
            .property("is-outgoing", td_message.is_outgoing)
            .property("can-be-edited", td_message.can_be_edited)
            .property(
                "can-be-deleted-only-for-self",
                td_message.can_be_deleted_only_for_self,
            )
            .property(
                "can-be-deleted-for-all-users",
                td_message.can_be_deleted_for_all_users,
            )
            .property(
                "sending-state",
                td_message
                    .sending_state
                    .map(model::BoxedMessageSendingState),
            )
            .property("date", td_message.date)
            .property(
                "interaction-info",
                model::MessageInteractionInfo::from(td_message.interaction_info),
            )
            .property(
                "forward-info",
                td_message
                    .forward_info
                    .map(|forward_info| model::MessageForwardInfo::new(chat, forward_info)),
            )
            .property(
                "reply-to",
                td_message.reply_to.map(model::BoxedMessageReplyTo),
            )
            .build();

        let imp = obj.imp();

        imp.content
            .replace(model::BoxedMessageContent(td_message.content));
        imp.is_edited.set(td_message.edit_date > 0);

        obj
    }

    pub(crate) fn chat_(&self) -> model::Chat {
        self.chat().unwrap()
    }

    pub(crate) fn handle_update(&self, update: tdlib::enums::Update) {
        use tdlib::enums::Update::*;

        match update {
            MessageContent(data) => {
                let new_content = model::BoxedMessageContent(data.new_content);
                self.set_content(new_content);
            }
            MessageEdited(data) => self.set_is_edited(data.edit_date > 0),
            MessageInteractionInfo(data) => self.interaction_info().update(data.interaction_info),
            _ => {}
        }
    }

    pub(crate) async fn delete(&self, revoke: bool) -> Result<(), tdlib::types::Error> {
        let chat = self.chat_();
        tdlib::functions::delete_messages(
            chat.id(),
            vec![self.id()],
            revoke,
            chat.session_().client_().id(),
        )
        .await
    }

    fn set_content(&self, content: model::BoxedMessageContent) {
        if self.content() == content {
            return;
        }
        self.imp().content.replace(content);
        self.notify_content();
    }

    fn set_is_edited(&self, is_edited: bool) {
        if self.is_edited() == is_edited {
            return;
        }
        self.imp().is_edited.set(is_edited);
        self.notify_is_edited();
    }

    pub(crate) fn sender_name_expression(&self) -> gtk::Expression {
        match self.sender() {
            MessageSender::User(user) => {
                let user_expression = gtk::ConstantExpression::new(user);
                expressions::user_display_name(&user_expression)
            }
            MessageSender::Chat(chat) => gtk::ConstantExpression::new(chat)
                .chain_property::<model::Chat>("title")
                .upcast(),
        }
    }

    pub(crate) fn sender_display_name_expression(&self) -> gtk::Expression {
        if self.chat_().is_own_chat() || self.chat_().is_replies_chat() {
            self.forward_info()
                .map(|forward_info| forward_info.origin())
                .map(|forward_origin| match forward_origin {
                    model::MessageForwardOrigin::User(user) => {
                        let user_expression = gtk::ObjectExpression::new(&user);
                        expressions::user_display_name(&user_expression)
                    }
                    model::MessageForwardOrigin::Chat { chat, .. }
                    | model::MessageForwardOrigin::Channel { chat, .. } => {
                        gtk::ConstantExpression::new(chat)
                            .chain_property::<model::Chat>("title")
                            .upcast()
                    }
                    model::MessageForwardOrigin::HiddenUser { sender_name }
                    | model::MessageForwardOrigin::MessageImport { sender_name } => {
                        gtk::ConstantExpression::new(sender_name).upcast()
                    }
                })
                .unwrap_or_else(|| self.sender_display_name_expression())
        } else {
            self.sender_name_expression()
        }
    }
}
