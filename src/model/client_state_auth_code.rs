use std::cell::Cell;
use std::cell::RefCell;

use glib::clone;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;

use crate::model;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::ClientStateAuthCode)]
    pub(crate) struct ClientAuthStateCode {
        pub(super) countdown_source_id: RefCell<Option<glib::SourceId>>,
        #[property(get, set, construct_only)]
        pub(super) auth: glib::WeakRef<model::ClientStateAuth>,
        #[property(get, set, construct)]
        pub(super) data: RefCell<Option<model::BoxedAuthorizationStateWaitCode>>,
        #[property(get, explicit_notify)]
        pub(super) countdown: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ClientAuthStateCode {
        const NAME: &'static str = "ClientStateAuthCode";
        type Type = super::ClientStateAuthCode;
    }

    impl ObjectImpl for ClientAuthStateCode {
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
            self.obj()
                .connect_data_notify(|obj| obj.imp().update_code_resend_state());
        }
    }

    impl ClientAuthStateCode {
        pub(super) fn set_countdown(&self, countdown: i32) {
            let obj = &*self.obj();
            if obj.countdown() == countdown {
                return;
            }
            self.countdown.set(countdown);
            obj.notify_countdown();
        }

        pub(super) fn update_code_resend_state(&self) {
            let obj = &*self.obj();

            // Always stop the resend countdown first.
            self.stop_code_next_type_countdown();

            let code_info = obj.data().unwrap().0.code_info;
            if code_info.next_type.is_some() && code_info.timeout > 0 {
                self.set_countdown(code_info.timeout);

                let source_id = glib::timeout_add_seconds_local(
                    1,
                    clone!(
                        #[weak]
                        obj,
                        #[upgrade_or]
                        glib::ControlFlow::Break,
                        move || {
                            let imp = obj.imp();

                            imp.set_countdown(obj.countdown() - 1);
                            glib::ControlFlow::from(if obj.countdown() == 0 {
                                imp.stop_code_next_type_countdown();
                                false
                            } else {
                                true
                            })
                        }
                    ),
                );
                self.countdown_source_id.replace(Some(source_id));
            }
        }

        pub(super) fn stop_code_next_type_countdown(&self) {
            if let Some(source_id) = self.countdown_source_id.take() {
                source_id.remove();
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct ClientStateAuthCode(ObjectSubclass<imp::ClientAuthStateCode>);
}

impl ClientStateAuthCode {
    pub(crate) fn new(
        auth: &model::ClientStateAuth,
        data: tdlib::types::AuthorizationStateWaitCode,
    ) -> Self {
        glib::Object::builder()
            .property("auth", auth)
            .property("data", model::BoxedAuthorizationStateWaitCode(data))
            .build()
    }

    pub(crate) fn auth_(&self) -> model::ClientStateAuth {
        self.auth().unwrap()
    }

    pub(crate) async fn send_code(&self, code: String) -> Result<(), tdlib::types::Error> {
        match tdlib::functions::check_authentication_code(code, self.auth_().client_().id()).await {
            Ok(_) => {
                self.imp().stop_code_next_type_countdown();
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to authenticate: {e:?}");
                Err(e)
            }
        }
    }

    pub(crate) async fn resend_auth_code(&self) -> Result<(), tdlib::types::Error> {
        match tdlib::functions::resend_authentication_code(self.auth_().client_().id()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                log::error!("Failed to resend auth code: {e:?}");

                if e.code == 8 {
                    // Sometimes the user may get a FLOOD_WAIT when he/she wants to resend the
                    // authorization code. But then tdlib blocks the resend function for the
                    // user, but does not inform us about it by sending an
                    // 'AuthorizationState::WaitCode'. Consequently, the user interface would
                    // still indicate that we are allowed to resend the code. However, we
                    // always get code 8 when we try, indicating that resending does not work.
                    // In this case, we automatically disable the resend feature.
                    self.imp().stop_code_next_type_countdown();
                    self.data().unwrap().0.code_info.next_type = None;
                    self.notify_data();
                }
                Err(e)
            }
        }
    }
}
