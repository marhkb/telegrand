use adw::prelude::*;
use adw::subclass::prelude::AdwWindowImpl;
use gettextrs::gettext;
use glib::clone;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use tdlib::enums::UserType;
use tdlib::functions;
use tdlib::types::BasicGroupFullInfo;
use tdlib::types::SupergroupFullInfo;

use crate::expressions;
use crate::strings::ChatSubtitleString;
use crate::strings::UserStatusString;
use crate::tdlib::BasicGroup;
use crate::tdlib::Chat;
use crate::tdlib::ChatType;
use crate::tdlib::Supergroup;
use crate::tdlib::User;
use crate::utils::spawn;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/content-chat-info-window.ui")]
    pub(crate) struct ChatInfoWindow {
        pub(super) chat: OnceCell<Chat>,
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) subtitle_label: TemplateChild<gtk::Inscription>,
        #[template_child]
        pub(super) info_list: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatInfoWindow {
        const NAME: &'static str = "ContentChatInfoWindow";
        type Type = super::ChatInfoWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChatInfoWindow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<Chat>("chat")
                    .construct_only()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "chat" => self.chat.set(value.get().unwrap()).unwrap(),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "chat" => obj.chat().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_window();
        }
    }

    impl WidgetImpl for ChatInfoWindow {}
    impl WindowImpl for ChatInfoWindow {}
    impl AdwWindowImpl for ChatInfoWindow {}
}

glib::wrapper! {
    pub(crate) struct ChatInfoWindow(ObjectSubclass<imp::ChatInfoWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

impl ChatInfoWindow {
    pub(crate) fn new(parent_window: &Option<gtk::Window>, chat: &Chat) -> Self {
        glib::Object::builder()
            .property("transient-for", parent_window)
            .property("chat", chat)
            .build()
    }

    fn setup_window(&self) {
        let imp = self.imp();
        let chat_expression = Self::this_expression("chat");

        // Bind the name
        expressions::chat_display_name(&chat_expression).bind(
            &*imp.name_label,
            "label",
            Some(self),
        );

        let chat = self.chat().unwrap();
        match chat.type_() {
            ChatType::Private(user) => {
                // TODO: Add handle own chat;
                if !chat.is_own_chat() {
                    self.setup_user_info(user);
                }
            }
            ChatType::BasicGroup(basic_group) => {
                self.setup_basic_group_info(basic_group);
            }
            ChatType::Supergroup(supergroup) => {
                self.setup_supergroup_info(supergroup);
            }
            ChatType::Secret(secret) => {
                self.setup_user_info(secret.user());
            }
        }
    }

    fn setup_user_info(&self, user: &User) {
        let imp = self.imp();

        // Online status or bot label
        if let UserType::Bot(_) = user.type_().0 {
            imp.subtitle_label.set_text(Some(&gettext("bot")));
        } else {
            let status_string = gtk::ConstantExpression::new(UserStatusString::new(user.clone()));
            status_string
                .chain_property::<UserStatusString>("string")
                .bind(&*imp.subtitle_label, "text", Some(user));
        }

        // Phone number
        if !user.phone_number().is_empty() {
            let row = new_property_row(&gettext("Mobile"), &format!("+{}", &user.phone_number()));
            self.make_row_copyable(&row);
            imp.info_list.append(&row);
        }

        // Username
        if !user.username().is_empty() {
            let row = new_property_row(&gettext("Username"), &format!("@{}", &user.username()));
            self.make_row_copyable(&row);
            imp.info_list.append(&row);
        }

        self.update_info_list_visibility();
    }

    fn setup_group_member_count(&self) {
        let imp = self.imp();
        let chat = self.chat().unwrap();
        let subtitle_string =
            gtk::ConstantExpression::new(ChatSubtitleString::new(chat.clone(), false));
        subtitle_string
            .chain_property::<ChatSubtitleString>("subtitle")
            .bind(&*imp.subtitle_label, "text", Some(chat));
        self.update_info_list_visibility();
    }

    fn setup_basic_group_info(&self, basic_group: &BasicGroup) {
        let client_id = self.chat().unwrap().session().client_id();
        let basic_group_id = basic_group.id();

        // Member count
        self.setup_group_member_count();

        // Full info
        spawn(clone!(@weak self as obj => async move {
            let result = functions::get_basic_group_full_info(basic_group_id, client_id).await;
            match result {
                Ok(tdlib::enums::BasicGroupFullInfo::BasicGroupFullInfo(full_info)) => {
                    obj.setup_basic_group_full_info(full_info);
                }
                Err(e) => {
                    log::warn!("Failed to get basic group full info: {e:?}");
                }
            }
        }));
    }

    fn setup_basic_group_full_info(&self, basic_group_full_info: BasicGroupFullInfo) {
        let imp = self.imp();

        // Description
        if !basic_group_full_info.description.is_empty() {
            let row = new_property_row(&gettext("Description"), &basic_group_full_info.description);
            self.make_row_copyable(&row);
            imp.info_list.append(&row);
        }

        self.update_info_list_visibility();
    }

    fn setup_supergroup_info(&self, supergroup: &Supergroup) {
        let client_id = self.chat().unwrap().session().client_id();
        let supergroup_id = supergroup.id();
        let imp = self.imp();

        // Member count
        self.setup_group_member_count();

        // Link
        if !supergroup.username().is_empty() {
            let row = new_property_row(
                &gettext("Link"),
                &format!("https://t.me/{}", &supergroup.username()),
            );
            self.make_row_copyable(&row);
            imp.info_list.append(&row);
        }

        self.update_info_list_visibility();

        // Full info
        spawn(clone!(@weak self as obj => async move {
            let result = functions::get_supergroup_full_info(supergroup_id, client_id).await;
            match result {
                Ok(tdlib::enums::SupergroupFullInfo::SupergroupFullInfo(full_info)) => {
                    obj.setup_supergroup_full_info(full_info);
                }
                Err(e) => {
                    log::warn!("Failed to get supergroup full info: {e:?}");
                }
            }
        }));
    }

    fn setup_supergroup_full_info(&self, supergroup_full_info: SupergroupFullInfo) {
        let imp = self.imp();

        // Description
        if !supergroup_full_info.description.is_empty() {
            let row = new_property_row(&gettext("Description"), &supergroup_full_info.description);
            self.make_row_copyable(&row);
            imp.info_list.append(&row);
        }

        self.update_info_list_visibility();
    }

    fn update_info_list_visibility(&self) {
        let info_list = &self.imp().info_list;
        info_list.set_visible(info_list.first_child().is_some());
    }

    fn make_row_copyable(&self, action_row: &adw::ActionRow) {
        action_row.set_activatable(true);
        action_row.connect_activated(clone!(@weak self as obj => move |action_row| {
            action_row.clipboard().set_text(&action_row.title());

            let toast = adw::Toast::new(&gettext("Copied to clipboard"));
            obj.imp().toast_overlay.add_toast(toast);
        }));
    }

    pub(crate) fn chat(&self) -> Option<&Chat> {
        self.imp().chat.get()
    }
}

fn new_property_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    row.add_css_class("property");
    row
}
