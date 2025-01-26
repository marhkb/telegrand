use std::cell::RefCell;
use std::fs;
use std::sync::OnceLock;
use std::thread;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use gio::subclass::prelude::*;
use glib::clone;
use glib::subclass::Signal;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use indexmap::map::Entry;
use indexmap::IndexMap;

use crate::model;
use crate::types::ClientId;
use crate::utils;
use crate::APPLICATION_OPTS;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub(crate) struct ClientManager(pub(super) RefCell<IndexMap<i32, model::Client>>);

    #[glib::object_subclass]
    impl ObjectSubclass for ClientManager {
        const NAME: &'static str = "ClientManager";
        type Type = super::ClientManager;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ClientManager {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("client-removed")
                        .param_types([model::Client::static_type()])
                        .build(),
                    Signal::builder("client-logged-in")
                        .param_types([model::Client::static_type()])
                        .build(),
                    Signal::builder("update-notification-group")
                        .param_types([
                            model::BoxedUpdateNotificationGroup::static_type(),
                            model::ClientStateSession::static_type(),
                        ])
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            // ####################################################################################
            // # Start the tdlib thread.                                                          #
            // ####################################################################################
            obj.start_tdlib_thread();

            // ####################################################################################
            // # Load the sessions from the data directory.                                       #
            // ####################################################################################
            match analyze_data_dir() {
                Err(e) => panic!("Could not initialize data directory: {e}"),
                Ok(database_infos) => {
                    if database_infos.is_empty() {
                        obj.add_new_client(APPLICATION_OPTS.get().unwrap().test_dc);
                    } else {
                        let remove_if_auth = database_infos.len() > 1;
                        database_infos.into_iter().for_each(|database_info| {
                            obj.add_client(tdlib::create_client(), database_info, remove_if_auth);
                        });
                    }
                }
            }
        }
    }

    impl ListModelImpl for ClientManager {
        fn item_type(&self) -> glib::Type {
            model::Client::static_type()
        }

        fn n_items(&self) -> u32 {
            self.0.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.0
                .borrow()
                .get_index(position as usize)
                .map(|(_, client)| client.to_owned().upcast())
        }
    }
}

glib::wrapper! {
    pub(crate) struct ClientManager(ObjectSubclass<imp::ClientManager>) @implements gio::ListModel;
}

impl Default for ClientManager {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl ClientManager {
    /// Function that returns all currently logged in users.
    pub(crate) fn sessions(&self) -> Vec<model::ClientStateSession> {
        self.imp()
            .0
            .borrow()
            .values()
            .cloned()
            .filter_map(|client| client.state())
            .filter_map(|client_state| client_state.downcast::<model::ClientStateSession>().ok())
            .collect()
    }

    /// Function that returns all currently logged in users.
    pub(crate) fn logged_in_users(&self) -> Vec<model::User> {
        self.sessions()
            .iter()
            .map(model::ClientStateSession::me_)
            .collect()
    }

    pub(crate) fn first_client(&self) -> Option<model::Client> {
        self.imp().0.borrow().values().next().cloned()
    }

    pub(crate) fn client_by_directory_base_name(
        &self,
        directory_base_name: &str,
    ) -> Option<model::Client> {
        self.imp()
            .0
            .borrow()
            .values()
            .find(|client| directory_base_name == client.database_info().0.directory_base_name)
            .cloned()
    }

    /// This function is used to add a new session for a so far unknown account. This means it will
    /// go through the login process.
    pub(crate) fn add_new_client(&self, use_test_dc: bool) -> model::Client {
        self.add_client(
            tdlib::create_client(),
            model::DatabaseInfo {
                directory_base_name: generate_database_dir_base_name(),
                use_test_dc,
            },
            false,
        )
    }

    fn add_client(
        &self,
        id: ClientId,
        database_info: model::DatabaseInfo,
        remove_if_auth: bool,
    ) -> model::Client {
        let client = model::Client::new(self, remove_if_auth, id, database_info);
        let (position, _) = self.imp().0.borrow_mut().insert_full(
            id,
            // Important: Here, we basically say that we just want to wait for
            // `AuthorizationState::Ready` and skip the login process.
            client.clone(),
        );
        self.items_changed(position as u32, 0, 1);

        client
    }

    pub(super) fn on_client_logged_in(&self, client: &model::Client) {
        self.emit_by_name::<()>("client-logged-in", &[&client]);
    }

    pub(crate) fn connect_client_removed<F: Fn(&Self, &model::Client) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("client-removed", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let client = values[1].get::<model::Client>().unwrap();
            f(&obj, &client);

            None
        })
    }

    pub(crate) fn connect_client_logged_in<F: Fn(&Self, &model::Client) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("client-logged-in", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let client = values[1].get::<model::Client>().unwrap();
            f(&obj, &client);

            None
        })
    }

    pub(crate) fn connect_update_notification_group<
        F: Fn(&Self, model::BoxedUpdateNotificationGroup, &model::ClientStateSession) + 'static,
    >(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("update-notification-group", true, move |values| {
            let obj: &Self = values[0].get().unwrap();
            let notification_group: model::BoxedUpdateNotificationGroup = values[1].get().unwrap();
            let session: &model::ClientStateSession = values[2].get().unwrap();

            f(obj, notification_group, session);

            None
        })
    }

    pub(crate) fn remove_client(&self, client: &model::Client) {
        let mut list = self.imp().0.borrow_mut();
        if let Some((position, _, _)) = list.swap_remove_full(&client.id()) {
            drop(list);
            self.items_changed(position as u32, 1, 0);
            self.emit_by_name::<()>("client-removed", &[&client]);
        }
    }

    pub(crate) fn handle_update(&self, update: tdlib::enums::Update, id: ClientId) {
        let mut list = self.imp().0.borrow_mut();
        if let Entry::Occupied(entry) = list.entry(id) {
            if let tdlib::enums::Update::NotificationGroup(group) = update {
                let session = entry
                    .get()
                    .state()
                    .and_downcast::<model::ClientStateSession>()
                    .unwrap();

                self.emit_by_name::<()>(
                    "update-notification-group",
                    &[&model::BoxedUpdateNotificationGroup(group), &session],
                );
            } else {
                let client = entry.get().to_owned();
                drop(list);

                if matches!(
                    &update,
                    tdlib::enums::Update::AuthorizationState(state)
                        if state.authorization_state == tdlib::enums::AuthorizationState::Closed)
                {
                    client.remove();
                } else {
                    client.handle_update(update);
                }
            }
        }
    }

    fn start_tdlib_thread(&self) {
        let (sender, receiver) = async_channel::unbounded();
        glib::spawn_future_local(clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                while let Ok((update, client_id)) = receiver.recv().await {
                    obj.handle_update(update, client_id);
                }
            }
        ));

        thread::spawn(move || loop {
            if let Some((update, client_id)) = tdlib::receive() {
                glib::spawn_future(clone!(
                    #[strong]
                    sender,
                    async move { _ = sender.send((update, client_id)).await }
                ));
            }
        });
    }
}

/// This function analyzes the data directory.
///
/// First, it checks whether the directory exists. It will create it and return immediately if
/// it doesn't.
///
/// If the data directory exists, information about the sessions is gathered. This is reading the
/// recently used sessions file and checking the individual session's database directory.
fn analyze_data_dir() -> Result<Vec<model::DatabaseInfo>, anyhow::Error> {
    if !utils::data_dir().exists() {
        // Create the Telegrand data directory if it does not exist and return.
        fs::create_dir_all(utils::data_dir())?;
        return Ok(Vec::new());
    }

    // All directories with the result of reading the session info file.
    let database_infos = fs::read_dir(utils::data_dir())?
        // Remove entries with error
        .filter_map(|res| res.ok())
        // Only consider directories.
        .filter(|entry| entry.path().is_dir())
        // Only consider directories with a "*.binlog" file
        .filter_map(|entry| {
            if entry.path().join("td.binlog").is_file() {
                return Some((entry, false));
            } else if entry.path().join("td_test.binlog").is_file() {
                return Some((entry, true));
            }
            None
        })
        .map(|(entry, use_test_dc)| model::DatabaseInfo {
            directory_base_name: entry
                .path()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            use_test_dc,
        })
        .collect::<Vec<_>>();

    Ok(database_infos)
}

/// This function generates a new database directory name based on the current UNIX system time
/// (e.g. db1638487692420). In the very unlikely case that a name is already taken it tries to
/// append a number at the end.
fn generate_database_dir_base_name() -> String {
    let database_dir_base_name = format!(
        "db{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis()
    );

    // Just to be sure!
    if utils::data_dir().join(&database_dir_base_name).exists() {
        (2..)
            .map(|count| format!("{database_dir_base_name}_{count}"))
            .find(|alternative_base_name| !utils::data_dir().join(alternative_base_name).exists())
            .unwrap()
    } else {
        database_dir_base_name
    }
}
