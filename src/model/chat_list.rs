use std::cell::Cell;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use gettextrs::gettext;
use glib::clone;
use glib::Properties;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::model;
use crate::types::ChatId;
use crate::utils;

#[derive(Debug, Default, Clone, Copy, PartialEq, glib::Enum)]
#[enum_type(name = "ChatListType")]
pub(crate) enum ChatListType {
    #[default]
    Main,
    Archive,
    Folder,
}
impl From<ChatListType> for Option<tdlib::enums::ChatList> {
    fn from(list_type: ChatListType) -> Self {
        match list_type {
            ChatListType::Main => None,
            ChatListType::Archive => Some(tdlib::enums::ChatList::Archive),
            ChatListType::Folder => Some(tdlib::enums::ChatList::Folder(
                tdlib::types::ChatListFolder { chat_folder_id: 0 },
            )),
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Properties, Default)]
    #[properties(wrapper_type = super::ChatList)]
    pub(crate) struct ChatList {
        // order -> item
        pub(super) list: RefCell<BTreeMap<i64, model::ChatListItem>>,
        #[property(get, set, construct_only)]
        pub(super) session: glib::WeakRef<model::ClientStateSession>,
        #[property(get, set, construct_only)]
        pub(super) list_type: RefCell<model::BoxedChatListType>,
        #[property(get, set)]
        pub(super) icon: RefCell<String>,
        #[property(get, set)]
        pub(super) title: RefCell<String>,
        #[property(get, set)]
        pub(super) unread_chat_count: Cell<i32>,
        #[property(get, set)]
        pub(super) unread_message_count: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatList {
        const NAME: &'static str = "ChatList";
        type Type = super::ChatList;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ChatList {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                Self::derived_properties()
                    .iter()
                    .cloned()
                    .chain(Some(
                        glib::ParamSpecUInt::builder("len").read_only().build(),
                    ))
                    .collect::<Vec<_>>()
            })
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "len" => self.obj().len().to_value(),
                _ => self.derived_property(id, pspec),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.obj()
                .connect_items_changed(|obj, _, _, _| obj.notify("len"));
        }
    }

    impl ListModelImpl for ChatList {
        fn item_type(&self) -> glib::Type {
            model::ChatListItem::static_type()
        }

        fn n_items(&self) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.list
                .borrow()
                .iter()
                .nth(position as usize)
                .map(|(_, c)| c.upcast_ref())
                .cloned()
        }
    }
}

glib::wrapper! {
    pub(crate) struct ChatList(ObjectSubclass<imp::ChatList>)
        @implements gio::ListModel;
}

impl ChatList {
    pub(crate) fn new(
        session: &model::ClientStateSession,
        list_type: tdlib::enums::ChatList,
    ) -> Self {
        glib::Object::builder()
            .property("session", session)
            .property("list-type", model::BoxedChatListType(list_type))
            .build()
    }

    pub(crate) fn len(&self) -> u32 {
        self.n_items()
    }

    pub(crate) fn session_(&self) -> model::ClientStateSession {
        self.session().unwrap()
    }

    pub(crate) fn fetch(&self) {
        utils::spawn(clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                let result = tdlib::functions::load_chats(
                    Some(obj.list_type().0),
                    20,
                    obj.session_().client_().id(),
                )
                .await;

                if let Err(err) = result {
                    // Error 404 means that all chats have been loaded
                    if err.code != 404 {
                        log::error!("Received an error for LoadChats: {}", err.code);
                    }
                } else {
                    obj.fetch();
                }
            }
        ));
    }

    pub(crate) fn update_chat_position(
        &self,
        chat: &model::Chat,
        position: &tdlib::types::ChatPosition,
    ) {
        let imp = self.imp();
        let mut list = imp.list.borrow_mut();

        match find_chat_item_position(&list, chat.id()) {
            Some((old_pos, old_order)) => {
                let item = list.remove(&old_order).unwrap();

                drop(list);
                self.items_changed(old_pos as u32, 1, 0);

                if position.order != 0 {
                    item.update(position);

                    self.insert_item(imp.list.borrow_mut(), item, position.order);
                }
            }
            None => self.insert_item(
                list,
                model::ChatListItem::new(chat, position),
                position.order,
            ),
        }
    }

    fn insert_item(
        &self,
        mut list: RefMut<BTreeMap<i64, model::ChatListItem>>,
        item: model::ChatListItem,
        order: i64,
    ) {
        let client_id = item.chat().unwrap().id();

        // Invert the sign to have a descending order
        list.insert(-order, item);

        let position = find_chat_item_position(&list, client_id).unwrap().0;

        drop(list);
        self.items_changed(position as u32, 0, 1);
    }

    pub(crate) async fn delete(&self) -> anyhow::Result<()> {
        match self.list_type().0 {
            tdlib::enums::ChatList::Folder(tdlib::types::ChatListFolder { chat_folder_id }) => {
                tdlib::functions::delete_chat_folder(
                    chat_folder_id,
                    Vec::new(),
                    self.session_().client_().id(),
                )
                .await
                .map_err(|e| anyhow::Error::msg(e.message))
            }
            _ => Err(anyhow::Error::msg(gettext("Only folders can be deleted."))),
        }
    }
}

fn find_chat_item_position(
    list: &RefMut<BTreeMap<i64, model::ChatListItem>>,
    chat_id: ChatId,
) -> Option<(usize, i64)> {
    list.iter()
        .enumerate()
        .find(|(_, (_, item))| item.chat().unwrap().id() == chat_id)
        .map(|(pos, (order, _))| (pos, *order))
}
