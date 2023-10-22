use std::cell::Cell;
use std::cell::OnceCell;
use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use glib::clone;
use gtk::gio;
use gtk::glib;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;

use crate::expressions;
use crate::model;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/chat_history.ui")]
    pub(crate) struct ChatHistory {
        pub(super) chat: glib::WeakRef<model::Chat>,
        pub(super) chat_handlers: RefCell<Vec<glib::SignalHandlerId>>,
        pub(super) session_handlers: RefCell<Vec<glib::SignalHandlerId>>,
        pub(super) model: RefCell<Option<model::ChatHistoryModel>>,
        pub(super) message_menu: OnceCell<gtk::PopoverMenu>,
        pub(super) is_auto_scrolling: Cell<bool>,
        pub(super) is_loading_messages: Cell<bool>,
        pub(super) sticky: Cell<bool>,
        #[template_child]
        pub(super) window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(super) background: TemplateChild<ui::Background>,
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) list_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) chat_action_bar: TemplateChild<ui::ChatActionBar>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatHistory {
        const NAME: &'static str = "PaplChatHistory";
        type Type = super::ChatHistory;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("chat-history.view-info", None, move |widget, _, _| {
                widget.open_info_dialog();
            });
            klass.install_action("chat-history.scroll-down", None, move |widget, _, _| {
                widget.scroll_down();
            });
            klass.install_action(
                "chat-history.reply",
                Some("x"),
                move |widget, _, variant| {
                    let message_id = variant.and_then(|v| v.get()).unwrap();
                    widget.imp().chat_action_bar.reply_to_message_id(message_id);
                },
            );
            klass.install_action("chat-history.edit", Some("x"), move |widget, _, variant| {
                let message_id = variant.and_then(|v| v.get()).unwrap();
                widget.imp().chat_action_bar.edit_message_id(message_id);
            });
            klass.install_action_async(
                "chat-history.leave-chat",
                None,
                |widget, _, _| async move {
                    widget.show_leave_chat_dialog().await;
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChatHistory {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::Chat>("chat")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecBoolean::builder("sticky")
                        .read_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "chat" => {
                    let chat = value.get().unwrap();
                    obj.set_chat(chat);
                }
                "sticky" => obj.set_sticky(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "chat" => obj.chat().to_value(),
                "sticky" => obj.sticky().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_expressions();

            let adj = self.list_view.vadjustment().unwrap();
            adj.connect_value_changed(clone!(@weak obj => move |adj| {
                let imp = obj.imp();

                if imp.is_loading_messages.get() {
                    return;
                }

                if imp.is_auto_scrolling.get() {
                    if adj.value() + adj.page_size() >= adj.upper() {
                        imp.is_auto_scrolling.set(false);
                        obj.set_sticky(true);
                    }
                } else {
                    obj.set_sticky(adj.value() + adj.page_size() >= adj.upper());

                    if adj.value() >= adj.page_size() * 2.0 && adj.upper() > adj.page_size() * 2.0 {
                        return;
                    }

                    if let Some(model) = imp.model.borrow().as_ref() {
                        imp.is_loading_messages.set(true);

                        utils::spawn(clone!(@weak obj, @weak model => async move {
                            obj.imp().is_loading_messages.set(false);

                            if let Err(model::ChatHistoryError::Tdlib(e)) =
                                model.load_older_messages(2).await
                            {
                                log::warn!("Couldn't load more chat messages: {:?}", e);
                            }
                        }));
                    }
                }
            }));

            adj.connect_upper_notify(clone!(@weak obj => move |_| {
                if obj.sticky() || obj.imp().is_auto_scrolling.get() {
                    obj.scroll_down();
                }
            }));
        }
    }

    impl WidgetImpl for ChatHistory {
        fn direction_changed(&self, previous_direction: gtk::TextDirection) {
            let obj = self.obj();

            if obj.direction() == previous_direction {
                return;
            }

            if let Some(menu) = self.message_menu.get() {
                menu.set_halign(if obj.direction() == gtk::TextDirection::Rtl {
                    gtk::Align::End
                } else {
                    gtk::Align::Start
                });
            }
        }
    }

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
        glib::Object::new()
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

    fn open_info_dialog(&self) {
        if let Some(chat) = self.chat() {
            ui::ChatInfoWindow::new(&self.parent_window(), &chat).present();
        }
    }

    async fn show_leave_chat_dialog(&self) {
        if let Some(chat) = self.chat() {
            let dialog = adw::MessageDialog::new(
                Some(&self.parent_window().unwrap()),
                Some(&gettext("Leave chat?")),
                Some(&gettext("Do you want to leave this chat?")),
            );
            dialog.add_responses(&[("no", &gettext("_No")), ("yes", &gettext("_Yes"))]);
            dialog.set_default_response(Some("no"));
            dialog.set_close_response("no");
            dialog.set_response_appearance("yes", adw::ResponseAppearance::Destructive);

            if dialog.choose_future().await == "yes" {
                match tdlib::functions::leave_chat(chat.id(), chat.session_().client_().id()).await
                {
                    Ok(_) => {
                        // Unselect recently left chat
                        utils::ancestor::<_, ui::Sidebar>(self)
                            .set_selected_chat(Option::<model::Chat>::None);
                    }
                    Err(e) => log::warn!("Failed to leave chat: {:?}", e),
                }
            }
        }
    }

    fn parent_window(&self) -> Option<gtk::Window> {
        self.root()?.downcast().ok()
    }

    fn request_sponsored_message(
        &self,
        session: &model::ClientStateSession,
        chat_id: i64,
        list: &gio::ListStore,
    ) {
        utils::spawn(clone!(@weak session, @weak list => async move {
            match model::SponsoredMessage::request(chat_id, &session).await {
                Ok(sponsored_message) => {
                    if let Some(sponsored_message) = sponsored_message {
                        list.append(&sponsored_message);
                    }
                }
                Err(e) => {
                    if e.code != 404 {
                        log::warn!("Failed to request a SponsoredMessage: {:?}", e);
                    }
                }
            }
        }));
    }

    pub(crate) fn message_menu(&self) -> &gtk::PopoverMenu {
        self.imp().message_menu.get_or_init(|| {
            let menu = gtk::Builder::from_resource(
                "/app/drey/paper-plane/ui/session/content/message_menu.ui",
            )
            .object::<gtk::PopoverMenu>("menu")
            .unwrap();

            menu.set_halign(if self.direction() == gtk::TextDirection::Rtl {
                gtk::Align::End
            } else {
                gtk::Align::Start
            });

            menu
        })
    }

    pub(crate) fn handle_paste_action(&self) {
        self.imp().chat_action_bar.handle_paste_action();
    }

    pub(crate) fn chat(&self) -> Option<model::Chat> {
        self.imp().chat.upgrade()
    }

    pub(crate) fn set_chat(&self, chat: Option<&model::Chat>) {
        if self.chat().as_ref() == chat {
            return;
        }

        let imp = self.imp();

        if let Some(chat) = chat {
            self.action_set_enabled(
                "chat-history.leave-chat",
                match chat.chat_type() {
                    model::ChatType::BasicGroup(data) => {
                        data.status().0 != tdlib::enums::ChatMemberStatus::Left
                    }
                    model::ChatType::Supergroup(data) => {
                        data.status().0 != tdlib::enums::ChatMemberStatus::Left
                    }
                    _ => false,
                },
            );

            let model = model::ChatHistoryModel::new(chat);

            // Request sponsored message, if needed
            let list_view_model: gio::ListModel = if matches!(chat.chat_type(), model::ChatType::Supergroup(supergroup) if supergroup.is_channel())
            {
                let list = gio::ListStore::new::<gio::ListModel>();

                // We need to create a list here so that we can append the sponsored message
                // to the chat history in the GtkListView using a GtkFlattenListModel
                let sponsored_message_list = gio::ListStore::new::<model::SponsoredMessage>();
                list.append(&sponsored_message_list);
                self.request_sponsored_message(
                    &chat.session_(),
                    chat.id(),
                    &sponsored_message_list,
                );

                list.append(&model);

                gtk::FlattenListModel::new(Some(list)).upcast()
            } else {
                model.clone().upcast()
            };

            utils::spawn(clone!(@weak self as obj, @weak model => async move {
                let imp = obj.imp();

                imp.is_loading_messages.set(true);

                let scrollbar = imp.scrolled_window.vscrollbar();
                scrollbar.set_visible(false);

                let adj = imp.list_view.vadjustment().unwrap();
                adj.set_value(0.0);

                while adj.value() == 0.0 {
                    match model.load_older_messages(2).await {
                        Ok(can_load_more) => if !can_load_more {
                            break;
                        }
                        Err(e) => {
                            log::warn!("Couldn't load initial history messages: {}", e);
                            break;
                        }
                    }
                }

                scrollbar.set_visible(true);

                imp.is_loading_messages.set(false);
                obj.set_sticky(true);
            }));

            self.imp().background.set_chat_theme(chat.chat_theme());

            let new_message_handler =
                chat.connect_new_message(clone!(@weak self as obj => move |_, msg| {
                    if msg.is_outgoing() {
                        obj.imp().background.animate();
                    }
                }));

            let chat_theme_handler = chat.connect_notify_local(
                Some("theme-name"),
                clone!(@weak self as obj => move |chat, _| {
                    obj.imp().background.set_chat_theme(chat.chat_theme());
                }),
            );

            for old_handler in self
                .imp()
                .chat_handlers
                .replace(vec![new_message_handler, chat_theme_handler])
            {
                if let Some(old_chat) = imp.chat.upgrade() {
                    old_chat.disconnect(old_handler);
                }
            }

            let chat_themes_handler = chat.session_().connect_update_chat_themes(
                clone!(@weak self as obj, @weak chat => move || {
                    obj.imp().background.set_chat_theme(chat.chat_theme());
                }),
            );

            for old_handler in self
                .imp()
                .session_handlers
                .replace(vec![chat_themes_handler])
            {
                if let Some(old_chat) = imp.chat.upgrade() {
                    old_chat.disconnect(old_handler);
                }
            }

            let selection = gtk::NoSelection::new(Some(list_view_model));
            imp.list_view.set_model(Some(&selection));

            imp.model.replace(Some(model));
        }

        imp.chat.set(chat);
        self.notify("chat");
    }

    pub(crate) fn sticky(&self) -> bool {
        self.imp().sticky.get()
    }

    fn set_sticky(&self, sticky: bool) {
        if self.sticky() == sticky {
            return;
        }

        self.imp().sticky.set(sticky);
        self.notify("sticky");
    }

    fn scroll_down(&self) {
        let imp = self.imp();

        imp.is_auto_scrolling.set(true);

        imp.scrolled_window
            .emit_by_name::<bool>("scroll-child", &[&gtk::ScrollType::End, &false]);
    }
}
