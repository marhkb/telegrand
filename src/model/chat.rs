use std::cell::Cell;
use std::cell::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use glib::subclass::Signal;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::model;
use crate::types::ChatId;
use crate::types::MessageId;

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "ChatType")]
pub(crate) enum ChatType {
    Private(model::User),
    BasicGroup(model::BasicGroup),
    Supergroup(model::Supergroup),
    Secret(model::SecretChat),
}

impl ChatType {
    pub(crate) fn from_td_object(
        _type: &tdlib::enums::ChatType,
        session: &model::ClientStateSession,
    ) -> Self {
        use tdlib::enums::ChatType::*;

        match _type {
            Private(data) => {
                let user = session.user(data.user_id);
                Self::Private(user)
            }
            BasicGroup(data) => {
                let basic_group = session.basic_group(data.basic_group_id);
                Self::BasicGroup(basic_group)
            }
            Supergroup(data) => {
                let supergroup = session.supergroup(data.supergroup_id);
                Self::Supergroup(supergroup)
            }
            Secret(data) => {
                let secret_chat = session.secret_chat(data.secret_chat_id);
                Self::Secret(secret_chat)
            }
        }
    }

    pub(crate) fn user(&self) -> Option<model::User> {
        Some(match self {
            ChatType::Private(user) => user.to_owned(),
            ChatType::Secret(secret_chat) => secret_chat.user_(),
            _ => return None,
        })
    }

    pub(crate) fn basic_group(&self) -> Option<&model::BasicGroup> {
        Some(match self {
            ChatType::BasicGroup(basic_group) => basic_group,
            _ => return None,
        })
    }

    pub(crate) fn supergroup(&self) -> Option<&model::Supergroup> {
        Some(match self {
            ChatType::Supergroup(supergroup) => supergroup,
            _ => return None,
        })
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Properties, Default)]
    #[properties(wrapper_type = super::Chat)]
    pub(crate) struct Chat {
        pub(super) messages: RefCell<HashMap<i64, model::Message>>,
        #[property(get, set, construct_only)]
        pub(super) session: glib::WeakRef<model::ClientStateSession>,
        #[property(get, set, construct_only)]
        pub(super) id: Cell<ChatId>,
        #[property(get, set, construct_only)]
        pub(super) chat_type: OnceCell<ChatType>,
        #[property(get)]
        pub(super) block_list: RefCell<Option<model::BoxedBlockList>>,
        #[property(get)]
        pub(super) title: RefCell<String>,
        #[property(get)]
        pub(super) avatar: RefCell<Option<model::Avatar>>,
        #[property(get)]
        pub(super) last_read_outbox_message_id: Cell<MessageId>,
        #[property(get)]
        pub(super) is_marked_as_unread: Cell<bool>,
        #[property(get)]
        pub(super) last_message: RefCell<Option<model::Message>>,
        #[property(get)]
        pub(super) unread_mention_count: Cell<i32>,
        #[property(get)]
        pub(super) unread_count: Cell<i32>,
        #[property(get)]
        pub(super) draft_message: RefCell<Option<model::BoxedDraftMessage>>,
        #[property(get)]
        pub(super) notification_settings: RefCell<model::BoxedChatNotificationSettings>,
        #[property(get = Self::actions)]
        pub(super) actions: OnceCell<model::ChatActionList>,
        #[property(get)]
        pub(super) permissions: RefCell<model::BoxedChatPermissions>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Chat {
        const NAME: &'static str = "Chat";
        type Type = super::Chat;
    }

    impl ObjectImpl for Chat {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("new-message")
                        .param_types([model::Message::static_type()])
                        .build(),
                    Signal::builder("deleted-message")
                        .param_types([model::Message::static_type()])
                        .build(),
                ]
            })
        }

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

    impl Chat {
        pub(crate) fn actions(&self) -> model::ChatActionList {
            self.actions
                .get_or_init(|| model::ChatActionList::from(&*self.obj()))
                .to_owned()
        }
    }
}

glib::wrapper! {
    pub(crate) struct Chat(ObjectSubclass<imp::Chat>);
}

impl Chat {
    pub(crate) fn new(session: &model::ClientStateSession, td_chat: tdlib::types::Chat) -> Self {
        let obj: Self = glib::Object::builder()
            .property("session", session)
            .property("id", td_chat.id)
            .property(
                "chat-type",
                ChatType::from_td_object(&td_chat.r#type, session),
            )
            .build();

        let imp = obj.imp();

        imp.block_list
            .replace(td_chat.block_list.map(model::BoxedBlockList));
        imp.title.replace(td_chat.title);
        imp.avatar.replace(td_chat.photo.map(model::Avatar::from));
        imp.last_read_outbox_message_id
            .set(td_chat.last_read_outbox_message_id);
        imp.is_marked_as_unread.set(td_chat.is_marked_as_unread);
        imp.last_message.replace(
            td_chat
                .last_message
                .map(|message| model::Message::new(&obj, message)),
        );
        imp.unread_mention_count.set(td_chat.unread_mention_count);
        imp.unread_count.set(td_chat.unread_count);
        imp.draft_message
            .replace(td_chat.draft_message.map(model::BoxedDraftMessage));
        imp.notification_settings
            .replace(model::BoxedChatNotificationSettings(
                td_chat.notification_settings,
            ));
        imp.permissions
            .replace(model::BoxedChatPermissions(td_chat.permissions));

        obj
    }

    pub(crate) fn handle_update(&self, update: tdlib::enums::Update) {
        use tdlib::enums::Update::*;

        let imp = self.imp();

        match update {
            ChatAction(update) => {
                self.actions().handle_update(update);
                // TODO: Remove this at some point. Widgets should use the `items-changed` signal
                // for updating their state in the future.
                self.notify_actions();
            }
            ChatDraftMessage(update) => {
                self.set_draft_message(update.draft_message.map(model::BoxedDraftMessage));
            }
            ChatBlockList(update) => {
                self.set_block_list(update.block_list.map(model::BoxedBlockList))
            }
            ChatIsMarkedAsUnread(update) => self.set_marked_as_unread(update.is_marked_as_unread),
            ChatLastMessage(update) => {
                self.set_last_message(update.last_message.map(|m| model::Message::new(self, m)));
            }
            ChatNotificationSettings(update) => {
                self.set_notification_settings(model::BoxedChatNotificationSettings(
                    update.notification_settings,
                ));
            }
            ChatPermissions(update) => {
                self.set_permissions(model::BoxedChatPermissions(update.permissions))
            }
            ChatPhoto(update) => self.set_avatar(update.photo.map(Into::into)),
            ChatReadInbox(update) => self.set_unread_count(update.unread_count),
            ChatReadOutbox(update) => {
                self.set_last_read_outbox_message_id(update.last_read_outbox_message_id);
            }
            ChatTitle(update) => self.set_title(update.title),
            ChatUnreadMentionCount(update) => {
                self.set_unread_mention_count(update.unread_mention_count)
            }
            DeleteMessages(data) => {
                // FIXME: This should be removed after we notify opened and closed chats to TDLib
                // See discussion here: https://t.me/tdlibchat/65304
                if !data.from_cache {
                    let mut messages = imp.messages.borrow_mut();
                    let deleted_messages: Vec<model::Message> = data
                        .message_ids
                        .into_iter()
                        .filter_map(|id| messages.remove(&id))
                        .collect();

                    drop(messages);
                    for message in deleted_messages {
                        self.emit_by_name::<()>("deleted-message", &[&message]);
                    }
                }
            }
            MessageContent(ref data) => {
                if let Some(message) = self.message(data.message_id) {
                    message.handle_update(update);
                }
            }
            MessageEdited(ref data) => {
                if let Some(message) = self.message(data.message_id) {
                    message.handle_update(update);
                }
            }
            MessageInteractionInfo(ref data) => {
                if let Some(message) = self.message(data.message_id) {
                    message.handle_update(update);
                }
            }
            MessageSendSucceeded(data) => {
                let mut messages = imp.messages.borrow_mut();
                let old_message = messages.remove(&data.old_message_id);

                let message_id = data.message.id;
                let message = model::Message::new(self, data.message);
                messages.insert(message_id, message.clone());

                drop(messages);
                self.emit_by_name::<()>("deleted-message", &[&old_message]);
                self.emit_by_name::<()>("new-message", &[&message]);
            }
            NewMessage(data) => {
                let message_id = data.message.id;
                let message = model::Message::new(self, data.message);
                imp.messages
                    .borrow_mut()
                    .insert(message_id, message.clone());

                self.emit_by_name::<()>("new-message", &[&message]);
            }
            MessageMentionRead(update) => {
                self.set_unread_mention_count(update.unread_mention_count)
            }
            _ => {}
        }
    }

    pub(crate) fn session_(&self) -> model::ClientStateSession {
        self.session().unwrap()
    }

    pub(crate) fn is_blocked(&self) -> bool {
        matches!(
            self.block_list(),
            Some(model::BoxedBlockList(tdlib::enums::BlockList::Main))
        )
    }

    fn set_block_list(&self, block_list: Option<model::BoxedBlockList>) {
        if self.block_list() == block_list {
            return;
        }
        self.imp().block_list.replace(block_list);
        self.notify_block_list();
    }

    fn set_title(&self, title: String) {
        if self.title() == title {
            return;
        }
        self.imp().title.replace(title);
        self.notify_title();
    }

    fn set_avatar(&self, avatar: Option<model::Avatar>) {
        if self.avatar() == avatar {
            return;
        }
        self.imp().avatar.replace(avatar);
        self.notify_avatar();
    }

    fn set_last_read_outbox_message_id(&self, id: MessageId) {
        if self.last_read_outbox_message_id() == id {
            return;
        }
        self.imp().last_read_outbox_message_id.set(id);
        self.notify_last_read_outbox_message_id();
    }

    fn set_marked_as_unread(&self, is_marked_as_unread: bool) {
        if self.is_marked_as_unread() == is_marked_as_unread {
            return;
        }
        self.imp().is_marked_as_unread.set(is_marked_as_unread);
        self.notify_is_marked_as_unread();
    }

    fn set_last_message(&self, last_message: Option<model::Message>) {
        if self.last_message() == last_message {
            return;
        }
        self.imp().last_message.replace(last_message);
        self.notify_last_message();
    }

    fn set_unread_mention_count(&self, unread_mention_count: i32) {
        if self.unread_mention_count() == unread_mention_count {
            return;
        }
        self.imp().unread_mention_count.set(unread_mention_count);
        self.notify_unread_mention_count();
    }

    fn set_unread_count(&self, unread_count: i32) {
        if self.unread_count() == unread_count {
            return;
        }
        self.imp().unread_count.set(unread_count);
        self.notify_unread_count()
    }

    fn set_draft_message(&self, draft_message: Option<model::BoxedDraftMessage>) {
        if self.draft_message() == draft_message {
            return;
        }
        self.imp().draft_message.replace(draft_message);
        self.notify_draft_message();
    }

    fn set_notification_settings(
        &self,
        notification_settings: model::BoxedChatNotificationSettings,
    ) {
        if self.notification_settings() == notification_settings {
            return;
        }
        self.imp()
            .notification_settings
            .replace(notification_settings);
        self.notify_notification_settings();
    }

    pub(crate) fn is_own_chat(&self) -> bool {
        self.chat_type().user() == Some(self.session_().me_())
    }

    pub(crate) fn is_replies_chat(&self) -> bool {
        self.id() == 1271266957 || self.id() == 708513
    }

    fn set_permissions(&self, permissions: model::BoxedChatPermissions) {
        if self.permissions() == permissions {
            return;
        }
        self.imp().permissions.replace(permissions);
        self.notify_permissions();
    }

    pub(crate) fn connect_new_message<F: Fn(&Self, model::Message) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("new-message", true, move |values| {
            let obj = values[0].get().unwrap();
            let message = values[1].get().unwrap();
            f(obj, message);
            None
        })
    }

    pub(crate) fn connect_deleted_message<F: Fn(&Self, model::Message) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("deleted-message", true, move |values| {
            let obj = values[0].get().unwrap();
            let message = values[1].get().unwrap();
            f(obj, message);
            None
        })
    }

    /// Returns the `Message` of the specified id, if present in the cache.
    pub(crate) fn message(&self, id: MessageId) -> Option<model::Message> {
        self.imp().messages.borrow().get(&id).cloned()
    }

    /// Returns the `Message` of the specified id, if present in the cache. Otherwise it
    /// fetches it from the server and then it returns the result.
    pub(crate) async fn fetch_message(
        &self,
        id: MessageId,
    ) -> Result<model::Message, tdlib::types::Error> {
        if let Some(message) = self.message(id) {
            return Ok(message);
        }

        let client_id = self.session_().client_().id();
        let result = tdlib::functions::get_message(self.id(), id, client_id).await;

        result.map(|r| {
            let tdlib::enums::Message::Message(message) = r;

            self.imp()
                .messages
                .borrow_mut()
                .entry(id)
                .or_insert_with(|| model::Message::new(self, message))
                .clone()
        })
    }

    pub(crate) async fn get_chat_history(
        &self,
        from_id: MessageId,
        limit: i32,
    ) -> Result<Vec<model::Message>, tdlib::types::Error> {
        let client_id = self.session_().client_().id();
        let result =
            tdlib::functions::get_chat_history(self.id(), from_id, 0, limit, false, client_id)
                .await;

        let tdlib::enums::Messages::Messages(data) = result?;

        let mut messages = self.imp().messages.borrow_mut();
        let loaded_messages: Vec<model::Message> = data
            .messages
            .into_iter()
            .flatten()
            .map(|m| model::Message::new(self, m))
            .collect();

        for message in &loaded_messages {
            messages.insert(message.id(), message.clone());
        }

        Ok(loaded_messages)
    }

    pub(crate) async fn mark_as_read(&self) -> Result<(), tdlib::types::Error> {
        if let Some(message) = self.last_message() {
            tdlib::functions::view_messages(
                self.id(),
                vec![message.id()],
                None,
                true,
                self.session_().client_().id(),
            )
            .await?;
        }

        tdlib::functions::toggle_chat_is_marked_as_unread(
            self.id(),
            false,
            self.session_().client_().id(),
        )
        .await
    }

    pub(crate) async fn mark_as_unread(&self) -> Result<(), tdlib::types::Error> {
        tdlib::functions::toggle_chat_is_marked_as_unread(
            self.id(),
            true,
            self.session_().client_().id(),
        )
        .await
    }
}
