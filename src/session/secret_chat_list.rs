use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use indexmap::map::Entry;
use tdgrand::enums::Update;

use crate::session::SecretChat;
use crate::Session;

mod imp {
    use super::*;
    use glib::WeakRef;
    use indexmap::IndexMap;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default)]
    pub struct SecretChatList {
        pub list: RefCell<IndexMap<i32, SecretChat>>,
        pub session: WeakRef<Session>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SecretChatList {
        const NAME: &'static str = "SecretChatList";
        type Type = super::SecretChatList;
        type ParentType = glib::Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for SecretChatList {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "session",
                    "Session",
                    "The session relative to this list",
                    Session::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "session" => self.session.set(Some(&value.get().unwrap())),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "session" => obj.session().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl ListModelImpl for SecretChatList {
        fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
            SecretChat::static_type()
        }

        fn n_items(&self, _list_model: &Self::Type) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, _list_model: &Self::Type, position: u32) -> Option<glib::Object> {
            self.list
                .borrow()
                .get_index(position as usize)
                .map(|(_, i)| i.upcast_ref())
                .cloned()
        }
    }
}

glib::wrapper! {
    pub struct SecretChatList(ObjectSubclass<imp::SecretChatList>)
        @implements gio::ListModel;
}

impl SecretChatList {
    pub fn new(session: &Session) -> Self {
        glib::Object::new(&[("session", session)]).expect("Failed to create SecretChatList")
    }

    pub fn get(&self, id: &i32) -> Option<SecretChat> {
        let self_ = imp::SecretChatList::from_instance(self);
        self_.list.borrow().get(id).cloned()
    }

    pub fn handle_update(&self, update: &Update) {
        if let Update::SecretChat(data) = update {
            let self_ = imp::SecretChatList::from_instance(self);
            let mut list = self_.list.borrow_mut();

            match list.entry(data.secret_chat.id) {
                Entry::Occupied(entry) => entry.get().handle_update(update),
                Entry::Vacant(entry) => {
                    let user = self
                        .session()
                        .user_list()
                        .get_or_create_user(data.secret_chat.user_id);
                    let secret_chat = SecretChat::from_td_object(&data.secret_chat, &user);
                    entry.insert(secret_chat);

                    drop(list);

                    let position = (self_.list.borrow().len() - 1) as u32;
                    self.items_changed(position, 0, 1);
                }
            }
        }
    }

    pub fn session(&self) -> Session {
        let self_ = imp::SecretChatList::from_instance(self);
        self_.session.upgrade().unwrap()
    }
}
