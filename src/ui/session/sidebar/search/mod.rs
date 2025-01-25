mod item_row;
mod row;
mod section;
mod section_row;

use std::sync::OnceLock;

use gettextrs::gettext;
use glib::clone;
use glib::subclass::Signal;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

pub(crate) use self::item_row::ItemRow;
pub(crate) use self::row::Row;
pub(crate) use self::section::Section;
pub(crate) use self::section::SectionType;
pub(crate) use self::section_row::SectionRow;
use crate::model;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/sidebar/search/mod.ui")]
    pub(crate) struct Search {
        pub(super) session: glib::WeakRef<model::ClientStateSession>,
        #[template_child]
        pub(super) toolbar_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::NoSelection>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Search {
        const NAME: &'static str = "PaplSidebarSearch";
        type Type = super::Search;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.set_css_name("sidebarsearch");

            klass.install_action_async(
                "sidebar-search.clear-recent-chats",
                None,
                |widget, _, _| async move {
                    let session = widget.session().unwrap();
                    if let Err(e) =
                        tdlib::functions::clear_recently_found_chats(session.client_().id()).await
                    {
                        log::warn!("Failed to clear recently found chats: {:?}", e);
                    }

                    // Update search view
                    widget.search().await;
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Search {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![Signal::builder("close").build()])
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::ClientStateSession>("session")
                        .explicit_notify()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "session" => obj.set_session(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "session" => obj.session().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for Search {
        fn grab_focus(&self) -> bool {
            self.search_entry.grab_focus();
            true
        }
    }
}

glib::wrapper! {
    pub(crate) struct Search(ObjectSubclass<imp::Search>)
        @extends gtk::Widget;
}

#[gtk::template_callbacks]
impl Search {
    pub(crate) fn reset(&self) {
        let imp = self.imp();
        if imp.search_entry.text().is_empty() {
            // Update recently found chats
            utils::spawn(clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    obj.search().await;
                }
            ));
        } else {
            // Reset the search entry. This will also start the search
            // for getting the recently found chats.
            imp.search_entry.set_text("");
        }
    }

    pub(crate) fn session(&self) -> Option<model::ClientStateSession> {
        self.imp().session.upgrade()
    }

    pub(crate) fn set_session(&self, session: Option<&model::ClientStateSession>) {
        if self.session().as_ref() == session {
            return;
        }
        self.imp().session.set(session);
        self.notify("session");
    }

    #[template_callback]
    async fn search(&self) {
        let imp = self.imp();
        let session = self.session().unwrap();
        let query = imp.search_entry.text().to_string();
        let list = gio::ListStore::new::<glib::Object>();
        let mut found_chat_ids: Vec<i64> = vec![];

        const MAX_KNOWN_CHATS: i32 = 50;

        imp.selection.set_model(Some(&list));
        list.connect_items_changed(clone!(
            #[weak(rename_to = obj)]
            self,
            move |list, _, _, _| {
                obj.imp()
                    .stack
                    .set_visible_child_name(if list.n_items() > 0 {
                        "results"
                    } else {
                        "empty"
                    });
            }
        ));

        // Show the results page prematurely, so that we don't show the empty page
        // before even starting the search.
        imp.stack.set_visible_child_name("results");

        if !query.is_empty()
            && gettext("Saved Messages")
                .to_lowercase()
                .contains(&query.to_lowercase())
        {
            let own_user_id = session.me_().id();
            if let Some(own_chat) = session.try_chat(own_user_id) {
                list.append(&Section::new(SectionType::Chats));

                found_chat_ids.push(own_user_id);
                list.append(&own_chat);
            }
        }

        // Search chats locally (or get the recently found chats if the query is empty)
        match tdlib::functions::search_chats(query.clone(), 30, session.client_().id()).await {
            Ok(tdlib::enums::Chats::Chats(mut data)) if !data.chat_ids.is_empty() => {
                list.append(&Section::new(if query.is_empty() {
                    SectionType::Recent
                } else {
                    SectionType::Chats
                }));

                let chats: Vec<model::Chat> = data
                    .chat_ids
                    .iter()
                    .filter(|id| !found_chat_ids.contains(id))
                    .map(|id| session.chat(*id))
                    .collect();

                found_chat_ids.append(&mut data.chat_ids);
                list.extend_from_slice(&chats);
            }
            Err(e) => {
                log::warn!("Error searching chats: {:?}", e);
            }
            _ => {}
        }

        // Show the empty page if there are no results after the first part of the search
        if list.n_items() == 0 {
            imp.stack.set_visible_child_name("empty");
        }

        // If the query is empty, we can stop the search here as we just need the
        // recently found chats that the previous search call returned.
        if query.is_empty() {
            return;
        }

        // Search known chats on server
        match tdlib::functions::search_chats_on_server(
            query.clone(),
            MAX_KNOWN_CHATS - found_chat_ids.len() as i32,
            session.client_().id(),
        )
        .await
        {
            Ok(tdlib::enums::Chats::Chats(data)) if !data.chat_ids.is_empty() => {
                if found_chat_ids.is_empty() {
                    list.append(&Section::new(SectionType::Chats));
                }

                let chats: Vec<model::Chat> = data
                    .chat_ids
                    .into_iter()
                    .filter_map(|id| {
                        if found_chat_ids.contains(&id) {
                            None
                        } else {
                            found_chat_ids.push(id);
                            Some(session.chat(id))
                        }
                    })
                    .collect();

                list.extend_from_slice(&chats);
            }
            Err(e) => {
                log::warn!("Error searching chats on server: {:?}", e);
            }
            _ => {}
        }

        if found_chat_ids.len() as i32 >= MAX_KNOWN_CHATS {
            return;
        }

        // Search contacts
        match tdlib::functions::search_contacts(
            query.clone(),
            MAX_KNOWN_CHATS - found_chat_ids.len() as i32,
            session.client_().id(),
        )
        .await
        {
            Ok(tdlib::enums::Users::Users(data)) if !data.user_ids.is_empty() => {
                if found_chat_ids.is_empty() {
                    list.append(&Section::new(SectionType::Chats));
                }

                let users: Vec<model::User> = data
                    .user_ids
                    .into_iter()
                    .filter_map(|id| {
                        // The user IDs are the same as their respective private chat IDs,
                        // so we can just check for chat IDs here.
                        if found_chat_ids.contains(&id) {
                            None
                        } else {
                            found_chat_ids.push(id);
                            Some(session.user(id))
                        }
                    })
                    .collect();

                list.extend_from_slice(&users);
            }
            Err(e) => {
                log::warn!("Error searching contacts: {:?}", e);
            }
            _ => {}
        }

        // Search public chats
        match tdlib::functions::search_public_chats(query, session.client_().id()).await {
            Ok(tdlib::enums::Chats::Chats(data)) if !data.chat_ids.is_empty() => {
                list.append(&Section::new(SectionType::Global));

                let chats: Vec<model::Chat> = data
                    .chat_ids
                    .into_iter()
                    .filter_map(|id| {
                        if found_chat_ids.contains(&id) {
                            None
                        } else {
                            Some(session.chat(id))
                        }
                    })
                    .collect();

                list.extend_from_slice(&chats);
            }
            Err(e) => {
                log::warn!("Error searching public chats: {:?}", e);
            }
            _ => {}
        }
    }

    #[template_callback]
    async fn list_activate(&self, position: u32) {
        let item = self.imp().selection.item(position).unwrap();
        let session = self.session().unwrap();
        let sidebar = utils::ancestor::<_, ui::Sidebar>(self);

        if let Some(chat) = item.downcast_ref::<model::Chat>() {
            sidebar.set_selected_chat(Some(chat));

            if let Err(e) =
                tdlib::functions::add_recently_found_chat(chat.id(), session.client_().id()).await
            {
                log::warn!("Failed to add recently found chat: {:?}", e);
            }
        } else if let Some(user) = item.downcast_ref::<model::User>() {
            // TODO
            // session.select_chat(chat);

            if let Err(e) =
                tdlib::functions::add_recently_found_chat(user.id(), session.client_().id()).await
            {
                log::warn!("Failed to add recently found chat: {:?}", e);
            }
        } else {
            log::warn!("Unexpected item type: {:?}", item);
        }

        self.emit_by_name::<()>("close", &[]);
    }
}
