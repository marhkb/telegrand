use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, CompositeTemplate};

use crate::session::content::{ChatActionBar, ChatInfoDialog, ItemRow};
use crate::tdlib::{Chat, ChatType, SponsoredMessage};
use crate::utils::spawn;
use crate::{expressions, Session};

mod imp {
    use super::*;
    use adw::subclass::prelude::BinImpl;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-chat-history.ui")]
    pub(crate) struct ChatHistory {
        pub(super) compact: Cell<bool>,
        pub(super) chat: RefCell<Option<Chat>>,
        pub(super) message_menu: OnceCell<gtk::PopoverMenu>,
        #[template_child]
        pub(super) window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(super) list_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) chat_action_bar: TemplateChild<ChatActionBar>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatHistory {
        const NAME: &'static str = "ContentChatHistory";
        type Type = super::ChatHistory;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            ItemRow::static_type();
            Self::bind_template(klass);

            klass.install_action("chat-history.view-info", None, move |widget, _, _| {
                widget.open_info_dialog();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChatHistory {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoolean::new(
                        "compact",
                        "Compact",
                        "Wheter a compact view is used or not",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpecObject::new(
                        "chat",
                        "Chat",
                        "The chat currently shown",
                        Chat::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
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
                "compact" => {
                    let compact = value.get().unwrap();
                    self.compact.set(compact);
                }
                "chat" => {
                    let chat = value.get().unwrap();
                    obj.set_chat(chat);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "chat" => obj.chat().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.setup_expressions();

            let adj = self.list_view.vadjustment().unwrap();
            adj.connect_value_changed(clone!(@weak obj => move |adj| {
                obj.load_older_messages(adj);
            }));
        }
    }

    impl WidgetImpl for ChatHistory {}
    impl BinImpl for ChatHistory {}
}

glib::wrapper! {
    pub(crate) struct ChatHistory(ObjectSubclass<imp::ChatHistory>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for ChatHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatHistory {
    pub(crate) fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create ChatHistory")
    }

    fn setup_expressions(&self) {
        let chat_expression = Self::this_expression("chat");

        // Chat title
        expressions::chat_display_name(&chat_expression).bind(
            &*self.imp().window_title,
            "title",
            Some(self),
        );
    }

    pub(crate) fn focus_message_entry(&self) {
        self.imp().chat_action_bar.focus_message_entry();
    }

    fn load_older_messages(&self, adj: &gtk::Adjustment) {
        if adj.value() < adj.page_size() * 2.0 || adj.upper() <= adj.page_size() * 2.0 {
            if let Some(chat) = self.chat() {
                spawn(clone!(@weak chat => async move {
                    chat.history().load_older_messages().await;
                }));
            }
        }
    }

    fn open_info_dialog(&self) {
        if let Some(chat) = self.chat() {
            ChatInfoDialog::new(&self.parent_window(), &chat).present();
        }
    }

    fn parent_window(&self) -> Option<gtk::Window> {
        self.root()?.downcast().ok()
    }

    fn request_sponsored_message(&self, session: &Session, chat_id: i64, list: &gio::ListStore) {
        spawn(clone!(@weak session, @weak list => async move {
            match SponsoredMessage::request(chat_id, &session).await {
                Ok(sponsored_message) => list.append(&sponsored_message),
                Err(e) => if e.code != 404 {
                    log::warn!("Failed to request a SponsoredMessage: {:?}", e);
                },
            }
        }));
    }

    pub(crate) fn message_menu(&self) -> &gtk::PopoverMenu {
        self.imp().message_menu.get_or_init(|| {
            gtk::Builder::from_resource("/com/github/melix99/telegrand/ui/message-menu.ui")
                .object::<gtk::PopoverMenu>("menu")
                .unwrap()
        })
    }

    pub(crate) fn handle_paste_action(&self) {
        self.imp().chat_action_bar.handle_paste_action();
    }

    pub(crate) fn chat(&self) -> Option<Chat> {
        self.imp().chat.borrow().clone()
    }

    pub(crate) fn set_chat(&self, chat: Option<Chat>) {
        if self.chat() == chat {
            return;
        }

        let imp = self.imp();

        if let Some(ref chat) = chat {
            // Request sponsored message, if needed
            let chat_history: gio::ListModel = if matches!(chat.type_(), ChatType::Supergroup(supergroup) if supergroup.is_channel())
            {
                let list = gio::ListStore::new(gio::ListModel::static_type());

                // We need to create a list here so that we can append the sponsored message
                // to the chat history in the GtkListView using a GtkFlattenListModel
                let sponsored_message_list = gio::ListStore::new(SponsoredMessage::static_type());
                list.append(&sponsored_message_list);
                self.request_sponsored_message(&chat.session(), chat.id(), &sponsored_message_list);

                list.append(chat.history());

                gtk::FlattenListModel::new(Some(&list)).upcast()
            } else {
                chat.history().to_owned().upcast()
            };

            let selection = gtk::NoSelection::new(Some(&chat_history));
            imp.list_view.set_model(Some(&selection));
        }

        imp.chat.replace(chat);
        self.notify("chat");

        let adj = imp.list_view.vadjustment().unwrap();
        self.load_older_messages(&adj);
    }
}
