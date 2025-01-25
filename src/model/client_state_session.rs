use std::cell::OnceCell;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use glib::clone;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;

use crate::model;
use crate::types::ChatId;
use crate::types::SecretChatId;
use crate::types::UserId;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Properties, Default)]
    #[properties(wrapper_type = super::ClientStateSession)]
    pub(crate) struct ClientStateSession {
        pub(super) chats: RefCell<HashMap<i64, model::Chat>>,
        pub(super) users: RefCell<HashMap<i64, model::User>>,
        pub(super) basic_groups: RefCell<HashMap<i64, model::BasicGroup>>,
        pub(super) supergroups: RefCell<HashMap<i64, model::Supergroup>>,
        pub(super) secret_chats: RefCell<HashMap<i32, model::SecretChat>>,
        pub(super) downloading_files:
            RefCell<HashMap<i32, Vec<async_channel::Sender<tdlib::types::File>>>>,

        #[property(get, set, construct_only)]
        pub(super) client: glib::WeakRef<model::Client>,
        #[property(get)]
        pub(super) me: glib::WeakRef<model::User>,
        #[property(get = Self::main_chat_list)]
        pub(super) main_chat_list: OnceCell<model::ChatList>,
        #[property(get = Self::archive_chat_list)]
        pub(super) archive_chat_list: OnceCell<model::ChatList>,
        #[property(get = Self::chat_folder_list)]
        pub(super) chat_folder_list: OnceCell<model::ChatFolderList>,
        #[property(get)]
        pub(super) private_chats_notification_settings:
            RefCell<model::BoxedScopeNotificationSettings>,
        #[property(get)]
        pub(super) group_chats_notification_settings:
            RefCell<model::BoxedScopeNotificationSettings>,
        #[property(get)]
        pub(super) channel_chats_notification_settings:
            RefCell<model::BoxedScopeNotificationSettings>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ClientStateSession {
        const NAME: &'static str = "ClientStateSession";
        type Type = super::ClientStateSession;
    }

    impl ObjectImpl for ClientStateSession {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            utils::spawn(clone!(
                #[weak]
                obj,
                async move {
                    if let Err(e) = tdlib::functions::set_option(
                        "notification_group_count_max".to_string(),
                        Some(tdlib::enums::OptionValue::Integer(
                            tdlib::types::OptionValueInteger { value: 5 },
                        )),
                        obj.client_().id(),
                    )
                    .await
                    {
                        log::warn!(
                            "Error setting the notification_group_count_max option: {:?}",
                            e
                        );
                    }

                    obj.fetch_chats();
                }
            ));
        }
    }

    impl ClientStateSession {
        /// Returns the main chat list.
        pub(crate) fn main_chat_list(&self) -> model::ChatList {
            self.main_chat_list
                .get_or_init(|| model::ChatList::new(&self.obj(), tdlib::enums::ChatList::Main))
                .to_owned()
        }

        /// Returns the list of archived chats.
        pub(crate) fn archive_chat_list(&self) -> model::ChatList {
            self.archive_chat_list
                .get_or_init(|| model::ChatList::new(&self.obj(), tdlib::enums::ChatList::Archive))
                .to_owned()
        }

        pub(crate) fn chat_folder_list(&self) -> model::ChatFolderList {
            self.chat_folder_list
                .get_or_init(|| model::ChatFolderList::from(&*self.obj()))
                .to_owned()
        }
    }
}

glib::wrapper! {
    pub(crate) struct ClientStateSession(ObjectSubclass<imp::ClientStateSession>);
}

impl ClientStateSession {
    pub(crate) fn new(client: &model::Client, me: tdlib::types::User) -> Self {
        let obj: Self = glib::Object::builder().property("client", client).build();
        obj.imp().me.set(Some(&obj.upsert_user(me)));
        obj
    }

    pub(crate) fn client_(&self) -> model::Client {
        self.client().unwrap()
    }

    pub(crate) fn me_(&self) -> model::User {
        self.me().unwrap()
    }

    /// Returns the `model::Chat` of the specified id, if present.
    pub(crate) fn try_chat(&self, id: ChatId) -> Option<model::Chat> {
        self.imp().chats.borrow().get(&id).cloned()
    }

    /// Returns the `model::Chat` of the specified id. Panics if the chat is not present.
    ///
    /// Note that TDLib guarantees that types are always returned before their ids,
    /// so if you use an id returned by TDLib, it should be expected that the
    /// relative `model::Chat` exists in the list.
    pub(crate) fn chat(&self, id: ChatId) -> model::Chat {
        self.try_chat(id)
            .expect("Failed to get expected model::Chat")
    }

    pub(crate) fn upsert_user(&self, td_user: tdlib::types::User) -> model::User {
        let mut users = self.imp().users.borrow_mut();

        match users.entry(td_user.id) {
            Entry::Occupied(entry) => {
                let user = entry.get();
                user.update(td_user);
                user.to_owned()
            }
            Entry::Vacant(entry) => {
                let user = model::User::new(self, td_user);
                entry.insert(user.clone());
                user
            }
        }
    }

    /// Returns the `model::User` of the specified id. Panics if the user is not present.
    ///
    /// Note that TDLib guarantees that types are always returned before their ids,
    /// so if you use an id returned by TDLib, it should be expected that the
    /// relative `model::User` exists in the list.
    pub(crate) fn user(&self, user_id: UserId) -> model::User {
        self.imp().users.borrow().get(&user_id).unwrap().clone()
    }

    /// Returns the `model::BasicGroup` of the specified id. Panics if the basic group is not present.
    ///
    /// Note that TDLib guarantees that types are always returned before their ids,
    /// so if you use an id returned by TDLib, it should be expected that the
    /// relative `model::BasicGroup` exists in the list.
    pub(crate) fn basic_group(&self, basic_group_id: i64) -> model::BasicGroup {
        self.imp()
            .basic_groups
            .borrow()
            .get(&basic_group_id)
            .expect("Failed to get expected model::BasicGroup")
            .clone()
    }

    /// Returns the `model::Supergroup` of the specified id. Panics if the supergroup is not present.
    ///
    /// Note that TDLib guarantees that types are always returned before their ids,
    /// so if you use an id returned by TDLib, it should be expected that the
    /// relative `model::Supergroup` exists in the list.
    pub(crate) fn supergroup(&self, supergroup_id: i64) -> model::Supergroup {
        self.imp()
            .supergroups
            .borrow()
            .get(&supergroup_id)
            .expect("Failed to get expected model::Supergroup")
            .clone()
    }

    /// Returns the `model::SecretChat` of the specified id. Panics if the secret chat is not present.
    ///
    /// Note that TDLib guarantees that types are always returned before their ids,
    /// so if you use an id returned by TDLib, it should be expected that the
    /// relative `model::SecretChat` exists in the list.
    pub(crate) fn secret_chat(&self, id: SecretChatId) -> model::SecretChat {
        self.imp()
            .secret_chats
            .borrow()
            .get(&id)
            .expect("Failed to get expected model::SecretChat")
            .clone()
    }

    pub(crate) fn fetch_chats(&self) {
        self.archive_chat_list().fetch();
        self.main_chat_list().fetch();
    }

    /// Fetches the contacts of the user.
    pub(crate) async fn fetch_contacts(&self) -> Result<Vec<model::User>, tdlib::types::Error> {
        let result = tdlib::functions::get_contacts(self.client_().id()).await;

        result.map(|data| {
            let tdlib::enums::Users::Users(users) = data;
            users.user_ids.into_iter().map(|id| self.user(id)).collect()
        })
    }

    fn set_private_chats_notification_settings(
        &self,
        settings: model::BoxedScopeNotificationSettings,
    ) {
        if self.private_chats_notification_settings() == settings {
            return;
        }
        self.imp()
            .private_chats_notification_settings
            .replace(settings);
        self.notify_private_chats_notification_settings();
    }

    fn set_group_chats_notification_settings(
        &self,
        settings: model::BoxedScopeNotificationSettings,
    ) {
        if self.group_chats_notification_settings() == settings {
            return;
        }
        self.imp()
            .group_chats_notification_settings
            .replace(settings);
        self.notify_group_chats_notification_settings();
    }

    fn set_channel_chats_notification_settings(
        &self,
        settings: model::BoxedScopeNotificationSettings,
    ) {
        if self.channel_chats_notification_settings() == settings {
            return;
        }
        self.imp()
            .channel_chats_notification_settings
            .replace(settings);
        self.notify_channel_chats_notification_settings();
    }

    /// Downloads a file of the specified id. This will only return when the file
    /// downloading has completed or has failed.
    pub(crate) async fn download_file(
        &self,
        file_id: i32,
    ) -> Result<tdlib::types::File, tdlib::types::Error> {
        let client_id = self.client_().id();
        let result = tdlib::functions::download_file(file_id, 5, 0, 0, true, client_id).await;

        result.map(|data| {
            let tdlib::enums::File::File(file) = data;
            file
        })
    }

    /// Downloads a file of the specified id and calls a closure every time there's an update
    /// about the progress or when the download has completed.
    pub(crate) fn download_file_with_updates<F: Fn(tdlib::types::File) + 'static>(
        &self,
        file_id: i32,
        f: F,
    ) {
        let (sender, receiver) = async_channel::unbounded::<tdlib::types::File>();

        glib::spawn_future_local(async move {
            while let Ok(file) = receiver.recv().await {
                if !file.local.is_downloading_active {
                    break;
                }
                f(file);
            }
        });

        let mut downloading_files = self.imp().downloading_files.borrow_mut();
        match downloading_files.entry(file_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().push(sender);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![sender]);

                let client_id = self.client_().id();
                utils::spawn(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    async move {
                        let result =
                            tdlib::functions::download_file(file_id, 5, 0, 0, false, client_id)
                                .await;
                        match result {
                            Ok(tdlib::enums::File::File(file)) => {
                                obj.handle_file_update(file);
                            }
                            Err(e) => {
                                log::warn!("Error downloading a file: {:?}", e);
                            }
                        }
                    }
                ));
            }
        }
    }

    pub(crate) fn cancel_download_file(&self, file_id: i32) {
        let client_id = self.client_().id();
        utils::spawn(async move {
            if let Err(e) = tdlib::functions::cancel_download_file(file_id, false, client_id).await
            {
                log::warn!("Error canceling a file: {:?}", e);
            }
        });
    }

    fn handle_chat_position_update(
        &self,
        chat: &model::Chat,
        position: &tdlib::types::ChatPosition,
    ) {
        use tdlib::enums::ChatList::*;

        match &position.list {
            Main => {
                self.main_chat_list().update_chat_position(chat, position);
            }
            Archive => {
                self.archive_chat_list()
                    .update_chat_position(chat, position);
            }
            Folder(data) => {
                self.chat_folder_list()
                    .get_or_create(data.chat_folder_id)
                    .update_chat_position(chat, position);
            }
        }
    }

    fn handle_file_update(&self, file: tdlib::types::File) {
        let mut downloading_files = self.imp().downloading_files.borrow_mut();
        if let Entry::Occupied(mut entry) = downloading_files.entry(file.id) {
            // Keep only the senders with which it was possible to send successfully.
            // It is indeed possible that the object that created the sender and receiver and
            // attached it to the default main context has been disposed in the meantime.
            // This is problematic if it is now tried to upgrade a weak reference of this object in
            // the receiver closure.
            // It will either panic directly if `@default-panic` is used or the sender will return
            // an error in the `SyncSender::send()` function if
            // `default-return glib::Continue(false)` is used. In the latter case, the Receiver
            // will be detached from the main context, which will cause the sending to fail.
            entry.get_mut();
            // .retain(|sender| sender.send(file.clone()).is_ok());

            if !file.local.is_downloading_active || entry.get().is_empty() {
                entry.remove();
            }
        }
    }

    pub(crate) fn handle_update(&self, update: tdlib::enums::Update) {
        use tdlib::enums::Update::*;

        match update {
            NewChat(data) => {
                // No need to update the chat positions here, tdlib sends
                // the correct chat positions in other updates later
                let chat = model::Chat::new(self, data.chat);
                self.imp().chats.borrow_mut().insert(chat.id(), chat);
            }
            ChatTitle(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatPhoto(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatPermissions(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatLastMessage(ref data) => {
                let chat = self.chat(data.chat_id);
                for position in &data.positions {
                    self.handle_chat_position_update(&chat, position);
                }
                chat.handle_update(update);
            }
            ChatPosition(ref data) => {
                self.handle_chat_position_update(&self.chat(data.chat_id), &data.position)
            }
            ChatReadInbox(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatReadOutbox(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatDraftMessage(ref data) => {
                let chat = self.chat(data.chat_id);
                for position in &data.positions {
                    self.handle_chat_position_update(&chat, position);
                }
                chat.handle_update(update);
            }
            ChatFolders(_) => self.chat_folder_list().handle_update(update),
            ChatNotificationSettings(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatUnreadMentionCount(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatBlockList(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatIsMarkedAsUnread(ref data) => self.chat(data.chat_id).handle_update(update),
            DeleteMessages(ref data) => self.chat(data.chat_id).handle_update(update),
            ChatAction(ref data) => self.chat(data.chat_id).handle_update(update),
            MessageContent(ref data) => self.chat(data.chat_id).handle_update(update),
            MessageEdited(ref data) => self.chat(data.chat_id).handle_update(update),
            MessageMentionRead(ref data) => self.chat(data.chat_id).handle_update(update),
            MessageSendSucceeded(ref data) => self.chat(data.message.chat_id).handle_update(update),
            NewMessage(ref data) => self.chat(data.message.chat_id).handle_update(update),
            BasicGroup(data) => {
                let mut basic_groups = self.imp().basic_groups.borrow_mut();
                match basic_groups.entry(data.basic_group.id) {
                    Entry::Occupied(entry) => entry.get().update(data.basic_group),
                    Entry::Vacant(entry) => {
                        entry.insert(model::BasicGroup::from(data.basic_group));
                    }
                }
            }
            File(update) => {
                self.handle_file_update(update.file);
            }
            ScopeNotificationSettings(update) => {
                use tdlib::enums::NotificationSettingsScope::*;

                let settings = model::BoxedScopeNotificationSettings(update.notification_settings);
                match update.scope {
                    PrivateChats => {
                        self.set_private_chats_notification_settings(settings);
                    }
                    GroupChats => {
                        self.set_group_chats_notification_settings(settings);
                    }
                    ChannelChats => {
                        self.set_channel_chats_notification_settings(settings);
                    }
                }
            }
            SecretChat(data) => {
                let mut secret_chats = self.imp().secret_chats.borrow_mut();
                match secret_chats.entry(data.secret_chat.id) {
                    Entry::Occupied(entry) => entry.get().update(data.secret_chat),
                    Entry::Vacant(entry) => {
                        entry.insert(model::SecretChat::new(
                            self.user(data.secret_chat.user_id),
                            data.secret_chat,
                        ));
                    }
                }
            }
            Supergroup(data) => {
                let mut supergroups = self.imp().supergroups.borrow_mut();
                match supergroups.entry(data.supergroup.id) {
                    Entry::Occupied(entry) => entry.get().update(data.supergroup),
                    Entry::Vacant(entry) => {
                        entry.insert(model::Supergroup::from(data.supergroup));
                    }
                }
            }
            UnreadChatCount(data) => {
                use tdlib::enums::ChatList::*;

                match data.chat_list {
                    Main => {
                        self.main_chat_list()
                            .set_unread_chat_count(data.unread_count);
                    }
                    Archive => {
                        self.archive_chat_list()
                            .set_unread_chat_count(data.unread_count);
                    }
                    Folder(data_) => {
                        self.chat_folder_list()
                            .get_or_create(data_.chat_folder_id)
                            .set_unread_chat_count(data.unread_count);
                    }
                }
            }
            UnreadMessageCount(data) => {
                use tdlib::enums::ChatList::*;

                match data.chat_list {
                    Main => {
                        self.main_chat_list()
                            .set_unread_message_count(data.unread_count);
                    }
                    Archive => {
                        self.archive_chat_list()
                            .set_unread_message_count(data.unread_count);
                    }
                    Folder(data_) => {
                        self.chat_folder_list()
                            .get_or_create(data_.chat_folder_id)
                            .set_unread_message_count(data.unread_count);
                    }
                }
            }
            User(data) => {
                self.upsert_user(data.user);
            }
            UserStatus(data) => {
                self.user(data.user_id).update_status(data.status);
            }
            _ => {}
        }
    }
}
