mod contacts_window;
mod content;
mod preferences_dialog;
mod row;
mod sidebar;
mod switcher;

use std::sync::OnceLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::gdk;
use gtk::glib;
use gtk::CompositeTemplate;

pub(crate) use self::contacts_window::ContactsWindow;
pub(crate) use self::contacts_window::Row as ContactRow;
pub(crate) use self::content::Background;
pub(crate) use self::content::ChatActionBar;
pub(crate) use self::content::ChatHistory;
pub(crate) use self::content::ChatHistoryRow;
pub(crate) use self::content::ChatInfoWindow;
pub(crate) use self::content::Content;
pub(crate) use self::content::EventRow;
pub(crate) use self::content::MediaPicture;
pub(crate) use self::content::MessageBase;
pub(crate) use self::content::MessageBaseExt;
pub(crate) use self::content::MessageBaseImpl;
pub(crate) use self::content::MessageBubble;
pub(crate) use self::content::MessageDocument;
pub(crate) use self::content::MessageDocumentStatusIndicator;
pub(crate) use self::content::MessageIndicators;
pub(crate) use self::content::MessageLabel;
pub(crate) use self::content::MessageLocation;
pub(crate) use self::content::MessagePhoto;
pub(crate) use self::content::MessageReply;
pub(crate) use self::content::MessageRow;
pub(crate) use self::content::MessageSticker;
pub(crate) use self::content::MessageText;
pub(crate) use self::content::MessageVenue;
pub(crate) use self::content::MessageVideo;
pub(crate) use self::content::SendMediaWindow;
pub(crate) use self::preferences_dialog::PreferencesDialog;
pub(crate) use self::row::Row;
pub(crate) use self::sidebar::Avatar as SidebarAvatar;
pub(crate) use self::sidebar::ChatFolderBar as SidebarChatFolderBar;
pub(crate) use self::sidebar::ChatFolderIcon as SidebarChatFolderIcon;
pub(crate) use self::sidebar::ChatFolderRow as SidebarChatFolderRow;
pub(crate) use self::sidebar::ChatFolderSelection as SidebarChatFolderSelection;
pub(crate) use self::sidebar::ChatList as SidebarChatList;
pub(crate) use self::sidebar::MiniThumbnail as SidebarMiniThumbnail;
pub(crate) use self::sidebar::Row as SidebarRow;
pub(crate) use self::sidebar::Search as SidebarSearch;
pub(crate) use self::sidebar::SearchItemRow as SidebarSearchItemRow;
pub(crate) use self::sidebar::SearchRow as SidebarSearchRow;
pub(crate) use self::sidebar::SearchSection as SidebarSearchSection;
pub(crate) use self::sidebar::SearchSectionRow as SidebarSearchSectionRow;
pub(crate) use self::sidebar::SearchSectionType as SidebarSearchSectionType;
pub(crate) use self::sidebar::Selection as SidebarSelection;
pub(crate) use self::sidebar::Sidebar;
pub(crate) use self::switcher::Switcher;
use crate::model;
use crate::types::ChatId;
use crate::ui;
use crate::utils;

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/mod.ui")]
    pub(crate) struct Session {
        pub(super) model: glib::WeakRef<model::ClientStateSession>,
        #[template_child]
        pub(super) split_view: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub(super) sidebar: TemplateChild<ui::Sidebar>,
        #[template_child]
        pub(super) content: TemplateChild<ui::Content>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Session {
        const NAME: &'static str = "Session";
        type Type = super::Session;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("session.show-preferences", None, move |session, _, _| {
                ui::PreferencesDialog::from(session).present(
                    session
                        .root()
                        .and_then(|r| r.downcast::<gtk::Window>().ok())
                        .as_ref(),
                );
            });
            klass.install_action("session.show-contacts", None, move |widget, _, _| {
                let parent = widget.root().and_then(|r| r.downcast().ok());
                let contacts = ui::ContactsWindow::new(parent.as_ref(), widget.clone());

                contacts.connect_contact_activated(clone!(
                    #[weak]
                    widget,
                    move |_, user_id| {
                        widget.select_chat(user_id);
                    }
                ));

                contacts.present();
            });

            klass.add_binding_action(
                gdk::Key::F,
                gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
                "session.begin-chats-search",
            );
            klass.install_action("session.begin-chats-search", None, |widget, _, _| {
                widget.begin_chats_search();
            });

            klass.add_binding_action(
                gdk::Key::v,
                gdk::ModifierType::CONTROL_MASK,
                "session.paste",
            );
            klass.install_action("session.paste", None, move |widget, _, _| {
                widget.handle_paste_action();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Session {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::ClientStateSession>("model")
                        .construct_only()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "model" => self.model.set(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "model" => self.obj().model().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Session {}
    impl BinImpl for Session {}
}

glib::wrapper! {
    pub(crate) struct Session(ObjectSubclass<imp::Session>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::ClientStateSession> for Session {
    fn from(model: &model::ClientStateSession) -> Self {
        glib::Object::builder().property("model", model).build()
    }
}

impl Session {
    pub(crate) fn model(&self) -> Option<model::ClientStateSession> {
        self.imp().model.upgrade()
    }

    pub(crate) fn select_chat(&self, id: ChatId) {
        match self.model().unwrap().try_chat(id) {
            Some(chat) => self.imp().sidebar.set_selected_chat(Some(&chat)),
            None => utils::spawn(clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    match tdlib::functions::create_private_chat(
                        id,
                        true,
                        obj.model().unwrap().client_().id(),
                    )
                    .await
                    {
                        Ok(tdlib::enums::Chat::Chat(data)) => obj
                            .imp()
                            .sidebar
                            .set_selected_chat(obj.model().unwrap().try_chat(data.id).as_ref()),
                        Err(e) => log::warn!("Failed to create private chat: {:?}", e),
                    }
                }
            )),
        }
    }

    pub(crate) fn handle_paste_action(&self) {
        self.imp().content.handle_paste_action();
    }

    pub(crate) fn begin_chats_search(&self) {
        let imp = self.imp();
        imp.split_view.set_show_content(false);
        imp.sidebar.begin_chats_search();
    }
}
