use std::cell::Cell;
use std::cell::RefCell;

use glib::clone;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::ChatList)]
    #[template(resource = "/app/drey/paper-plane/ui/session/sidebar/chat_list.ui")]
    pub(crate) struct ChatList {
        pub(super) marked_as_unread_handler_id: RefCell<Option<glib::SignalHandlerId>>,
        #[property(get, set = Self::set_selected_chat, nullable, explicit_notify)]
        pub(super) selected_chat: glib::WeakRef<model::Chat>,
        #[property(get, set)]
        pub(super) chat_list: glib::WeakRef<model::ChatList>,
        #[property(get, set)]
        pub(super) compact: Cell<bool>,
        #[template_child]
        pub(super) selection: TemplateChild<ui::SidebarSelection>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatList {
        const NAME: &'static str = "PaplSidebarChatList";
        type Type = super::ChatList;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            klass.set_css_name("chatlist");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChatList {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn dispose(&self) {
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for ChatList {}

    #[gtk::template_callbacks]
    impl ChatList {
        pub(crate) fn set_selected_chat(&self, selected_chat: Option<&model::Chat>) {
            let obj = &*self.obj();
            if obj.selected_chat().as_ref() == selected_chat {
                return;
            }

            if let Some(handler_id) = self.marked_as_unread_handler_id.take() {
                obj.selected_chat().unwrap().disconnect(handler_id);
            }

            match selected_chat {
                Some(chat) => {
                    let handler_id = chat.connect_notify_local(
                        Some("is-marked-as-unread"),
                        clone!(
                            #[weak(rename_to = obj)]
                            self,
                            move |chat, _| {
                                if chat.is_marked_as_unread() {
                                    obj.set_selected_chat(None);
                                }
                            }
                        ),
                    );
                    self.marked_as_unread_handler_id.replace(Some(handler_id));

                    self.selection.set_selected_chat(Some(chat));

                    if chat.is_marked_as_unread() {
                        utils::spawn(clone!(
                            #[weak]
                            chat,
                            async move {
                                if let Err(e) = chat.mark_as_read().await {
                                    log::warn!("Error on toggling chat's unread state: {e:?}");
                                }
                            }
                        ));
                    }
                }
                None => self.selection.set_selected_chat(None),
            }

            self.selected_chat.set(selected_chat);

            obj.notify_selected_chat();
        }

        #[template_callback]
        fn on_notify_chat_list(&self) {
            self.selection
                .set_selected_chat(self.obj().selected_chat().as_ref());
        }

        #[template_callback]
        fn list_activate(&self, pos: u32) {
            let obj = &*self.obj();
            self.selection.set_selected_position(pos);

            obj.set_selected_chat(self.selection.selected_chat());
            obj.activate_action("navigation.push", Some(&"content".to_variant()))
                .unwrap();
        }
    }
}

glib::wrapper! {
    pub(crate) struct ChatList(ObjectSubclass<imp::ChatList>)
        @extends gtk::Widget;
}
