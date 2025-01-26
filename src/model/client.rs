use std::cell::Cell;
use std::cell::OnceCell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;

use glib::clone;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::config;
use crate::model;
use crate::types::ClientId;
use crate::utils;
use crate::APPLICATION_OPTS;

/// A struct for storing information about a session's database.
#[derive(Clone, Debug)]
pub(crate) struct DatabaseInfo {
    // The base name of the database directory.
    pub(crate) directory_base_name: String,
    // Whether this database uses a test dc.
    pub(crate) use_test_dc: bool,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Client)]
    pub(crate) struct Client {
        pub(crate) queued_updates: RefCell<VecDeque<tdlib::enums::Update>>,
        #[property(get, set, construct_only)]
        pub(super) client_manager: glib::WeakRef<model::ClientManager>,
        #[property(get, set, construct_only)]
        pub(super) remove_if_auth: OnceCell<bool>,
        #[property(get, set, construct_only)]
        pub(super) id: OnceCell<ClientId>,
        #[property(get, set, construct_only)]
        pub(super) database_info: OnceCell<model::BoxedDatabaseInfo>,
        #[property(get)]
        pub(super) state: RefCell<Option<glib::Object>>,
        #[property(get, set)]
        pub(super) active: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Client {
        const NAME: &'static str = "Client";
        type Type = super::Client;
    }

    impl ObjectImpl for Client {
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

            utils::spawn(clone!(
                #[weak]
                obj,
                async move {
                    obj.init().await;
                }
            ));
            obj.set_state(model::ClientStateAuth::from(obj).upcast());
        }
    }
}

glib::wrapper! { pub(crate) struct Client(ObjectSubclass<imp::Client>); }

impl Client {
    pub(crate) fn new(
        client_manager: &model::ClientManager,
        remove_if_auth: bool,
        id: ClientId,
        database_info: model::DatabaseInfo,
    ) -> Self {
        glib::Object::builder()
            .property("client-manager", client_manager)
            .property("remove-if-auth", remove_if_auth)
            .property("id", id)
            .property("database-info", model::BoxedDatabaseInfo(database_info))
            .build()
    }

    pub(crate) fn client_manager_(&self) -> model::ClientManager {
        self.client_manager().unwrap()
    }

    async fn init(&self) {
        if let Err(e) = tdlib::functions::set_log_verbosity_level(
            if log::log_enabled!(log::Level::Trace) {
                5
            } else if log::log_enabled!(log::Level::Debug) {
                4
            } else if log::log_enabled!(log::Level::Info) {
                3
            } else if log::log_enabled!(log::Level::Warn) {
                2
            } else {
                0
            },
            self.id(),
        )
        .await
        {
            log::warn!("Error setting the tdlib log level: {:?}", e);
        }

        if let Err(e) = tdlib::functions::set_option(
            "notification_group_count_max".to_string(),
            Some(tdlib::enums::OptionValue::Integer(
                tdlib::types::OptionValueInteger { value: 5 },
            )),
            self.id(),
        )
        .await
        {
            log::warn!(
                "Error setting the notification_group_count_max option: {:?}",
                e
            );
        }

        // TODO: Hopefully we'll support animated emoji at some point
        if let Err(e) = tdlib::functions::set_option(
            "disable_animated_emoji".to_string(),
            Some(tdlib::enums::OptionValue::Boolean(
                tdlib::types::OptionValueBoolean { value: true },
            )),
            self.id(),
        )
        .await
        {
            log::warn!("Error disabling animated emoji: {:?}", e);
        }
    }

    pub(crate) fn is_logged_in(&self) -> bool {
        self.state()
            .and_downcast::<model::ClientStateSession>()
            .is_some()
    }

    pub(crate) async fn send_tdlib_parameters(&self) -> Result<(), tdlib::types::Error> {
        let system_language_code = {
            let locale = locale_config::Locale::current().to_string();
            if !locale.is_empty() {
                locale
            } else {
                "en_US".to_string()
            }
        };

        let database_info = &self.database_info().0;

        let database_directory = utils::data_dir()
            .join(&database_info.directory_base_name)
            .to_str()
            .expect("Data directory path is not a valid unicode string")
            .into();

        let application_opts = APPLICATION_OPTS.get().unwrap();

        tdlib::functions::set_tdlib_parameters(
            database_info.use_test_dc,
            database_directory,
            String::new(),
            String::new(),
            true,
            true,
            true,
            true,
            application_opts.client_id,
            application_opts.client_secret.to_string(),
            system_language_code,
            "Desktop".into(),
            String::new(),
            config::VERSION.into(),
            true,
            false,
            self.id(),
        )
        .await
    }

    fn set_state(&self, state: glib::Object) {
        if self.state().as_ref() == Some(&state) {
            return;
        }
        self.imp().state.replace(Some(state));
        self.notify_state();
    }

    pub(crate) async fn set_online(&self, online: bool) {
        if let Err(e) = tdlib::functions::set_option(
            "online".to_string(),
            Some(tdlib::enums::OptionValue::Boolean(
                tdlib::types::OptionValueBoolean { value: online },
            )),
            self.id(),
        )
        .await
        {
            log::error!(
                "Could not set online state of client '{}' to '{}': {}",
                self.id(),
                online,
                e.message
            );
        }
    }

    pub(crate) async fn log_out(&self) -> Result<(), tdlib::types::Error> {
        match tdlib::functions::log_out(self.id()).await {
            Ok(_) => {
                self.remove();
                Ok(())
            }
            Err(e) => {
                log::error!(
                    "Failed to log out client with id={}: {}",
                    self.id(),
                    e.message
                );
                Err(e)
            }
        }
    }

    pub(crate) fn remove(&self) {
        let database_path = utils::data_dir().join(self.database_info().0.directory_base_name);
        if fs::metadata(&database_path).is_ok() {
            if let Err(e) = fs::remove_dir_all(&database_path) {
                log::error!("Error on on removing database directory {database_path:?}: {e}");
            }
        }

        self.client_manager_().remove_client(self);
    }

    pub(crate) fn handle_update(&self, update: tdlib::enums::Update) {
        use tdlib::enums::AuthorizationState;
        use tdlib::enums::Update;

        match update {
            Update::AuthorizationState(state) => match state.authorization_state {
                AuthorizationState::WaitTdlibParameters => {
                    utils::spawn(clone!(
                        #[weak(rename_to = obj)]
                        self,
                        async move {
                            if let Err(e) = obj.send_tdlib_parameters().await {
                                log::error!("Failed sensing tdlib parameters: {e:?}");
                            }
                        }
                    ));
                }
                AuthorizationState::Ready => {
                    utils::spawn(clone!(
                        #[weak(rename_to = obj)]
                        self,
                        async move {
                            let tdlib::enums::User::User(me) =
                                tdlib::functions::get_me(obj.id()).await.unwrap();

                            let state = model::ClientStateSession::new(&obj, me);
                            while let Some(update) =
                                obj.imp().queued_updates.borrow_mut().pop_front()
                            {
                                state.handle_update(update);
                            }

                            obj.set_state(state.upcast());

                            obj.client_manager_().on_client_logged_in(&obj);
                        }
                    ));
                }
                AuthorizationState::Closing => {
                    self.set_state(model::ClientStateLoggingOut::default().upcast());
                }
                AuthorizationState::LoggingOut => {
                    log::info!("Logging out client {}", self.id());
                }
                _ => self
                    .state()
                    .and_downcast::<model::ClientStateAuth>()
                    .unwrap()
                    .handle_update(state),
            },
            _ => {
                let state = self.state().unwrap();

                match state.downcast_ref::<model::ClientStateSession>() {
                    Some(state) => state.handle_update(update),
                    None => self.imp().queued_updates.borrow_mut().push_back(update),
                }
            }
        }
    }
}
