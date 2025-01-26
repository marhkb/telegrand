use std::cell::Cell;
use std::cell::RefCell;

use glib::clone;
use glib::Properties;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::model;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Selection)]
    pub(crate) struct Selection {
        pub(super) item_position: Cell<u32>,
        pub(super) signal_handler: RefCell<Option<glib::SignalHandlerId>>,
        #[property(get, set = Self::set_model, explicit_notify)]
        pub(super) model: RefCell<Option<gio::ListModel>>,
        #[property(get, set = Self::set_selected_chat_list, explicit_notify)]
        pub(super) selected_chat_list: glib::WeakRef<model::ChatList>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Selection {
        const NAME: &'static str = "PaplSidebarChatFolderSelection";
        type Type = super::Selection;
        type Interfaces = (gio::ListModel, gtk::SelectionModel);
    }

    impl ObjectImpl for Selection {
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
            self.item_position.set(gtk::INVALID_LIST_POSITION)
        }

        fn dispose(&self) {
            self.disconnect_model_signal();
        }
    }

    impl ListModelImpl for Selection {
        fn item_type(&self) -> glib::Type {
            model::ChatList::static_type()
        }

        fn n_items(&self) -> u32 {
            self.model
                .borrow()
                .as_ref()
                .map(|m| m.n_items())
                .unwrap_or_default()
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.model.borrow().as_ref().and_then(|m| m.item(position))
        }
    }

    impl SelectionModelImpl for Selection {
        fn is_selected(&self, position: u32) -> bool {
            self.item_position.get() == position
        }

        fn selection_in_range(&self, _position: u32, _n_items: u32) -> gtk::Bitset {
            let result = gtk::Bitset::new_empty();
            let item_position = self.item_position.get();
            if item_position != gtk::INVALID_LIST_POSITION {
                result.add(item_position);
            }

            result
        }

        fn select_item(&self, position: u32, _: bool) -> bool {
            let chat_list = self.item(position).and_downcast::<model::ChatList>();
            self.set_selected_item_internal(chat_list.as_ref(), position);

            true
        }
    }

    impl Selection {
        fn set_model(&self, model: Option<gio::ListModel>) {
            let obj = &*self.obj();
            if obj.model() == model {
                return;
            }

            let n_items_before = self.n_items();
            self.disconnect_model_signal();

            let n_items = if let Some(ref model) = model {
                let handler = model.connect_items_changed(clone!(
                    #[weak]
                    obj,
                    move |_, p, r, a| {
                        obj.imp().model_items_changed(p, r, a);
                    }
                ));
                self.signal_handler.replace(Some(handler));

                model.n_items()
            } else {
                0
            };

            self.model.replace(model);

            obj.items_changed(0, n_items_before, n_items);

            obj.notify_model();
        }

        fn set_selected_chat_list(&self, chat_list: Option<&model::ChatList>) {
            let obj = &self.obj();
            let position = chat_list
                .as_ref()
                .map(|chat_list| obj.find_item_position(chat_list, 0, self.n_items()))
                .unwrap_or(gtk::INVALID_LIST_POSITION);

            self.set_selected_item_internal(chat_list, position);
        }

        fn set_selected_item_internal(&self, item: Option<&model::ChatList>, position: u32) {
            let obj = self.obj();

            self.selected_chat_list.set(item);

            let old_position = self.item_position.get();
            self.item_position.set(position);

            if old_position != gtk::INVALID_LIST_POSITION || position != gtk::INVALID_LIST_POSITION
            {
                if old_position == gtk::INVALID_LIST_POSITION {
                    obj.selection_changed(position, 1);
                } else if position == gtk::INVALID_LIST_POSITION {
                    obj.selection_changed(old_position.min(self.n_items() - 1), 1);
                } else if position < old_position {
                    obj.selection_changed(
                        position,
                        (old_position - position + 1).min(self.n_items() - position),
                    );
                } else {
                    obj.selection_changed(old_position, position - old_position + 1);
                }
            }

            obj.notify_selected_chat_list();
        }

        fn model_items_changed(&self, position: u32, removed: u32, added: u32) {
            let obj = &*self.obj();

            let item_position = self.item_position.get();

            if let Some(chat_list) = obj.selected_chat_list() {
                if item_position == gtk::INVALID_LIST_POSITION {
                    // Maybe the item got newly added
                    self.item_position.set(obj.find_item_position(
                        &chat_list,
                        position,
                        position + added,
                    ));
                } else if item_position < position {
                    // Nothing to do, position stays the same
                } else if item_position < position + removed {
                    let new_item_position =
                        obj.find_item_position(&chat_list, position, position + added);
                    self.item_position.set(new_item_position);

                    if new_item_position == gtk::INVALID_LIST_POSITION {
                        self.selected_chat_list.set(None);
                        obj.notify_selected_chat_list();
                    }
                } else {
                    self.item_position
                        .set((item_position as i64 + (added as i64 - removed as i64)) as u32);
                }
            }

            obj.items_changed(position, removed, added);
        }

        fn disconnect_model_signal(&self) {
            if let Some(model) = self.obj().model() {
                let handler = self.signal_handler.take().unwrap();
                model.disconnect(handler);
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct Selection(ObjectSubclass<imp::Selection>)
        @implements gio::ListModel, gtk::SelectionModel;
}

impl Selection {
    pub(crate) fn find_item_position(
        &self,
        chat_list: &model::ChatList,
        start_pos: u32,
        end_pos: u32,
    ) -> u32 {
        if let Some(model) = self.model() {
            for pos in start_pos..end_pos {
                if let Some(item) = model.item(pos) {
                    if item.downcast_ref::<model::ChatList>().unwrap() == chat_list {
                        return pos;
                    }
                }
            }
        }

        gtk::INVALID_LIST_POSITION
    }
}
