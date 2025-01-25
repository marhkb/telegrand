mod row;

use std::cell::OnceCell;
use std::sync::OnceLock;

use adw::subclass::prelude::*;
use glib::clone;
use glib::subclass::Signal;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::CompositeTemplate;

pub(crate) use self::row::Row;
use crate::model;
use crate::strings;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/contacts_window/mod.ui")]

    pub(crate) struct ContactsWindow {
        pub(super) session: OnceCell<ui::Session>,
        #[template_child]
        pub(super) sort_model: TemplateChild<gtk::SortListModel>,
        #[template_child]
        pub(super) list_view: TemplateChild<gtk::ListView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ContactsWindow {
        const NAME: &'static str = "PaplContactsWindow";
        type Type = super::ContactsWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ContactsWindow {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![Signal::builder("contact-activated")
                    .param_types([i64::static_type()])
                    .build()]
            })
        }
    }

    impl WidgetImpl for ContactsWindow {}
    impl WindowImpl for ContactsWindow {}
    impl AdwWindowImpl for ContactsWindow {}

    #[gtk::template_callbacks]
    impl ContactsWindow {
        #[template_callback]
        fn list_activate(&self, pos: u32) {
            let obj = self.obj();
            let user = self
                .list_view
                .model()
                .and_then(|model| model.item(pos))
                .and_downcast::<model::User>()
                .unwrap();

            obj.emit_by_name::<()>("contact-activated", &[&user.id()]);
            obj.close();
        }

        #[template_callback]
        fn user_display_name(user: &model::User) -> String {
            strings::user_display_name(user, true)
        }
    }
}

glib::wrapper! {
    pub(crate) struct ContactsWindow(ObjectSubclass<imp::ContactsWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

impl ContactsWindow {
    pub(crate) fn new(parent: Option<&gtk::Window>, session: ui::Session) -> Self {
        let obj: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();

        obj.imp().session.set(session).unwrap();

        utils::spawn(clone!(
            #[weak]
            obj,
            async move {
                obj.fetch_contacts().await;
            }
        ));

        obj
    }

    async fn fetch_contacts(&self) {
        let session = self.imp().session.get().unwrap();

        match session.model().unwrap().fetch_contacts().await {
            Ok(users) => {
                let list = gio::ListStore::new::<model::User>();
                list.splice(0, 0, &users);

                self.imp().sort_model.set_model(Some(&list));
            }
            Err(e) => {
                log::warn!("Error fetching contacts: {:?}", e)
            }
        }
    }

    pub(crate) fn connect_contact_activated<F: Fn(&Self, i64) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("contact-activated", true, move |values| {
            let obj = values[0].get().unwrap();
            let user_id = values[1].get().unwrap();
            f(obj, user_id);
            None
        })
    }
}
