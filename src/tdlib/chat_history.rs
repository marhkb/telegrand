use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use tdlib::enums::{self, Update};
use tdlib::functions;
use tdlib::types::Message as TelegramMessage;
use thiserror::Error;

use crate::tdlib::{Chat, ChatHistoryItem, ChatHistoryItemType, Message};

#[derive(Error, Debug)]
pub(crate) enum ChatHistoryError {
    #[error("The chat history is already loading messages")]
    AlreadyLoading,
    #[error("TDLib error: {0:?}")]
    Tdlib(tdlib::types::Error),
}

mod imp {
    use super::*;
    use glib::WeakRef;
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};
    use std::collections::{HashMap, VecDeque};

    #[derive(Debug, Default)]
    pub(crate) struct ChatHistory {
        pub(super) chat: WeakRef<Chat>,
        pub(super) is_loading: Cell<bool>,
        pub(super) list: RefCell<VecDeque<ChatHistoryItem>>,
        pub(super) message_map: RefCell<HashMap<i64, Message>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatHistory {
        const NAME: &'static str = "ChatHistory";
        type Type = super::ChatHistory;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ChatHistory {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<Chat>("chat")
                    .read_only()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "chat" => obj.chat().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl ListModelImpl for ChatHistory {
        fn item_type(&self) -> glib::Type {
            ChatHistoryItem::static_type()
        }

        fn n_items(&self) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.list
                .borrow()
                .get(position as usize)
                .map(glib::object::Cast::upcast_ref::<glib::Object>)
                .cloned()
        }
    }
}

glib::wrapper! {
    pub(crate) struct ChatHistory(ObjectSubclass<imp::ChatHistory>)
        @implements gio::ListModel;
}

impl ChatHistory {
    pub(crate) fn new(chat: &Chat) -> Self {
        let chat_history: ChatHistory = glib::Object::new();
        chat_history.imp().chat.set(Some(chat));
        chat_history
    }

    /// Loads older messages from this chat history.
    ///
    /// Returns `true` when more messages can be loaded.
    pub(crate) async fn load_older_messages(&self, limit: i32) -> Result<bool, ChatHistoryError> {
        let imp = self.imp();

        if imp.is_loading.get() {
            return Err(ChatHistoryError::AlreadyLoading);
        }

        let chat = self.chat();
        let client_id = chat.session().client_id();
        let chat_id = chat.id();
        let oldest_message_id = imp
            .list
            .borrow()
            .iter()
            .rev()
            .find_map(|item| item.message())
            .map(|m| m.id())
            .unwrap_or_default();

        imp.is_loading.set(true);

        let result =
            functions::get_chat_history(chat_id, oldest_message_id, 0, limit, false, client_id)
                .await;

        imp.is_loading.set(false);

        let enums::Messages::Messages(data) = result.map_err(ChatHistoryError::Tdlib)?;

        if data.messages.is_empty() {
            return Ok(false);
        }

        let messages = data.messages.into_iter().flatten().collect();
        self.append(messages);

        Ok(true)
    }

    pub(crate) fn message_by_id(&self, id: i64) -> Option<Message> {
        let imp = self.imp();
        imp.message_map.borrow().get(&id).cloned()
    }

    pub(crate) fn handle_update(&self, update: Update) {
        use Update::*;

        match update {
            DeleteMessages(update) => {
                if !update.from_cache {
                    for message_id in update.message_ids {
                        self.remove(message_id);
                    }
                }
            }
            MessageContent(ref update_) => self.handle_message_update(update_.message_id, update),
            MessageEdited(ref update_) => self.handle_message_update(update_.message_id, update),
            MessageSendSucceeded(update) => {
                self.remove(update.old_message_id);
                self.push_front(update.message);
            }
            NewMessage(update) => self.push_front(update.message),
            _ => {}
        }
    }

    fn handle_message_update(&self, message_id: i64, update: Update) {
        if let Some(message) = self.imp().message_map.borrow().get(&message_id) {
            message.handle_update(update);
        }
    }

    fn items_changed(&self, position: u32, removed: u32, added: u32) {
        let imp = self.imp();

        // Insert day dividers where needed
        let added = {
            let position = position as usize;
            let added = added as usize;

            let mut list = imp.list.borrow_mut();
            let mut previous_timestamp = if position + 1 < list.len() {
                list.get(position + 1)
                    .and_then(|item| item.message_timestamp())
            } else {
                None
            };
            let mut dividers: Vec<(usize, ChatHistoryItem)> = vec![];

            for (index, current) in list.range(position..position + added).enumerate().rev() {
                if let Some(current_timestamp) = current.message_timestamp() {
                    if Some(current_timestamp.ymd()) != previous_timestamp.as_ref().map(|t| t.ymd())
                    {
                        let divider_pos = position + index + 1;
                        dividers.push((
                            divider_pos,
                            ChatHistoryItem::for_day_divider(current_timestamp.clone()),
                        ));
                        previous_timestamp = Some(current_timestamp);
                    }
                }
            }

            let dividers_len = dividers.len();
            for (position, item) in dividers {
                list.insert(position, item);
            }

            (added + dividers_len) as u32
        };

        // Check and remove no more needed day divider after removing messages
        let removed = {
            let mut removed = removed as usize;

            if removed > 0 {
                let mut list = imp.list.borrow_mut();
                let position = position as usize;
                let item_before_removed = list.get(position);

                if let Some(ChatHistoryItemType::DayDivider(_)) =
                    item_before_removed.map(|i| i.type_())
                {
                    let item_after_removed = if position > 0 {
                        list.get(position - 1)
                    } else {
                        None
                    };

                    match item_after_removed.map(|item| item.type_()) {
                        None | Some(ChatHistoryItemType::DayDivider(_)) => {
                            list.remove(position + removed);

                            removed += 1;
                        }
                        _ => {}
                    }
                }
            }

            removed as u32
        };

        // Check and remove no more needed day divider after adding messages
        let (position, removed) = {
            let mut removed = removed;
            let mut position = position as usize;

            if added > 0 && position > 0 {
                let mut list = imp.list.borrow_mut();
                let last_added_timestamp = list.get(position).unwrap().message_timestamp().unwrap();
                let next_item = list.get(position - 1);

                if let Some(ChatHistoryItemType::DayDivider(date)) =
                    next_item.map(|item| item.type_())
                {
                    if date.ymd() == last_added_timestamp.ymd() {
                        list.remove(position - 1);

                        removed += 1;
                        position -= 1;
                    }
                }
            }

            (position as u32, removed)
        };

        self.upcast_ref::<gio::ListModel>()
            .items_changed(position, removed, added);
    }

    fn push_front(&self, message: TelegramMessage) {
        let imp = self.imp();

        let mut message_map = imp.message_map.borrow_mut();

        if let Entry::Vacant(entry) = message_map.entry(message.id) {
            let message = Message::new(message, &self.chat());

            entry.insert(message.clone());

            imp.list
                .borrow_mut()
                .push_front(ChatHistoryItem::for_message(message));

            // We always need to drop all references before handing over control. Else, we could end
            // up with a borrowing error somewhere else.
            drop(message_map);
            self.items_changed(0, 0, 1);
        }
    }

    fn append(&self, messages: Vec<TelegramMessage>) {
        let imp = self.imp();
        let chat = self.chat();
        let added = messages.len();

        imp.list.borrow_mut().reserve(added);

        for message in messages {
            let message = Message::new(message, &chat);

            imp.message_map
                .borrow_mut()
                .insert(message.id(), message.clone());

            imp.list
                .borrow_mut()
                .push_back(ChatHistoryItem::for_message(message));
        }

        let index = imp.list.borrow().len() - added;
        self.items_changed(index as u32, 0, added as u32);
    }

    fn remove(&self, message_id: i64) {
        let imp = self.imp();

        if let Some(message) = imp.message_map.borrow_mut().remove(&message_id) {
            // Put this in a block, so that we only need to borrow the list once and the runtime
            // borrow checker does not panic in Self::items_changed when it borrows the list again.
            let index = {
                let mut list = imp.list.borrow_mut();

                // The elements in this list are ordered. While the day dividers are ordered
                // only by their date time, the messages are additionally sorted by their id. We
                // can exploit this by applying a binary search.
                let index = list
                    .binary_search_by(|m| match m.type_() {
                        ChatHistoryItemType::Message(message) => message_id.cmp(&message.id()),
                        ChatHistoryItemType::DayDivider(date_time) => {
                            let ordering = glib::DateTime::from_unix_utc(message.date() as i64)
                                .unwrap()
                                .cmp(date_time);
                            if let Ordering::Equal = ordering {
                                // We found the day divider of the message. Therefore, the message
                                // must be among the following elements.
                                Ordering::Greater
                            } else {
                                ordering
                            }
                        }
                    })
                    .unwrap();

                list.remove(index);
                index as u32
            };

            self.items_changed(index, 1, 0);
        }
    }

    pub(crate) fn chat(&self) -> Chat {
        self.imp().chat.upgrade().unwrap()
    }
}
