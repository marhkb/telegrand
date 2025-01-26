use std::cell::RefCell;

use glib::clone;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::types::ChatId;
use crate::types::ClientId;
use crate::ui;
use crate::utils;
use crate::APPLICATION_OPTS;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::ClientManagerView)]
    #[template(resource = "/app/drey/paper-plane/ui/client_manager_view.ui")]
    pub(crate) struct ClientManagerView {
        pub(super) settings: utils::PaperPlaneSettings,
        #[property(get, nullable)]
        pub(super) client_manager: model::ClientManager,
        /// The client from `client_manager` we currently show to the user
        #[property(get, set = Self::set_active_client, explicit_notify)]
        pub(super) active_client: glib::WeakRef<model::Client>,
        #[property(get)]
        pub(super) last_used_session: RefCell<Option<String>>,
        #[template_child]
        pub(super) animated_bin: TemplateChild<ui::AnimatedBin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ClientManagerView {
        const NAME: &'static str = "PaplClientManagerView";
        type Type = super::ClientManagerView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ClientManagerView {
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

            let obj = &*self.obj();

            self.client_manager.connect_client_removed(clone!(
                #[weak]
                obj,
                move |_, client| {
                    if Some(client) == obj.active_client().as_ref() {
                        obj.restore_last_session()
                    }
                }
            ));

            obj.restore_last_session();
        }

        fn dispose(&self) {
            self.animated_bin.unparent();
        }
    }

    impl WidgetImpl for ClientManagerView {}

    #[gtk::template_callbacks]
    impl ClientManagerView {
        fn set_active_client(&self, client: &model::Client) {
            let obj = &*self.obj();

            let old_active_client = obj.active_client();
            if old_active_client.as_ref() == Some(client) {
                return;
            }

            if let Some(client) = old_active_client {
                client.set_active(false);
                utils::spawn(async move {
                    client.set_online(false).await;
                });
            }

            self.active_client.set(Some(client));
            obj.notify_active_client();
        }

        #[template_callback]
        fn on_notify_active_client(&self) {
            let obj = &*self.obj();

            let client = obj.active_client().unwrap();

            client.set_active(true);
            obj.set_active_client_online();

            if client.is_logged_in() {
                if let Err(e) = self.settings.set_string(
                    "last-used-session",
                    &client.database_info().0.directory_base_name,
                ) {
                    log::warn!("Could not save setting 'last-used-session': {e}");
                }
            }

            self.animated_bin.set_child(&ui::ClientView::from(&client));
        }
    }
}

glib::wrapper! {
    pub(crate) struct ClientManagerView(ObjectSubclass<imp::ClientManagerView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ClientManagerView {
    pub(crate) fn add_new_client(&self, use_test_dc: bool) {
        self.set_active_client(self.client_manager().add_new_client(use_test_dc));
    }

    fn restore_last_session(&self) {
        let imp = self.imp();

        let active_client = imp.client_manager.first_client().and_then(|first_client| {
            let directory_base_name = imp.settings.string("last-used-session");
            imp.client_manager
                .client_by_directory_base_name(directory_base_name.as_str())
                .or(Some(first_client))
        });

        match active_client {
            Some(client) => self.set_active_client(&client),
            None => self.add_new_client(APPLICATION_OPTS.get().unwrap().test_dc),
        }
    }

    /// Sets the online status for the active logged in client. This will be called from the
    /// application `Window` when its active state has changed.
    pub(crate) fn set_active_client_online(&self) {
        utils::spawn(clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                if let Some(client) = obj.active_client() {
                    client
                        .set_online(
                            obj.root()
                                .and_downcast::<gtk::Window>()
                                .unwrap()
                                .is_active(),
                        )
                        .await;
                }
            }
        ));
    }

    pub(crate) fn select_chat(&self, client_id: ClientId, chat_id: ChatId) {
        if self
            .active_client()
            .filter(|client| client.id() == client_id)
            .is_some()
        {
            let mut child = self.first_child();
            while let Some(ref c) = child {
                if let Some(session) = c.downcast_ref::<ui::Session>() {
                    session.select_chat(chat_id);
                    break;
                }

                child = c.first_child();
            }
        }
    }
}
