use std::cell::RefCell;

use futures::future;
use glib::clone;
use glib::subclass::InitializingObject;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::i18n::gettext_f;
use crate::model;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/app/drey/paper-plane/ui/login/phone_number.ui")]
    #[properties(wrapper_type = super::PhoneNumber)]
    pub(crate) struct PhoneNumber {
        pub(super) abort_handle: RefCell<Option<future::AbortHandle>>,
        #[property(get, set)]
        pub(super) model: glib::WeakRef<model::ClientStateAuthPhoneNumber>,
        #[template_child]
        pub(super) input: TemplateChild<ui::PhoneNumberInput>,
        #[template_child]
        pub(super) next_button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) qr_code_spinner: TemplateChild<gtk::Spinner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PhoneNumber {
        const NAME: &'static str = "PaplLoginPhoneNumber";
        type Type = super::PhoneNumber;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action_async(
                "login.phone-number.exit",
                None,
                |widget, _, _| async move {
                    widget.exit().await;
                },
            );

            klass.install_action_async(
                "login.phone-number.next",
                None,
                |widget, _, _| async move {
                    widget.next().await;
                },
            );

            klass.install_action_async(
                "login.phone-number.use-qr-code",
                None,
                |widget, _, _| async move {
                    widget.request_qr_code().await;
                },
            );
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PhoneNumber {
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
            self.setup_expressions();
        }

        fn dispose(&self) {
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for PhoneNumber {
        fn root(&self) {
            self.parent_root();
            self.obj().focus_input();
        }
    }

    #[gtk::template_callbacks]
    impl PhoneNumber {
        #[template_callback]
        fn on_notify_model(&self) {
            let obj = &*self.obj();

            if let Some(model) = obj.model() {
                obj.action_set_enabled(
                    "login.phone-number.exit",
                    !model
                        .auth_()
                        .client_()
                        .client_manager_()
                        .sessions()
                        .is_empty(),
                );

                utils::spawn(clone!(
                    #[weak]
                    obj,
                    #[weak]
                    model,
                    async move {
                        if let Err(e) = model.load_country_codes().await {
                            utils::show_toast(
                                &obj,
                                gettext_f(
                                    "Failed to load country codes: {error}",
                                    &[("error", &e.message)],
                                ),
                            );
                        }
                    }
                ));
            }
        }

        #[template_callback]
        fn on_input_activated(&self) {
            self.obj()
                .activate_action("login.phone-number.next", None)
                .unwrap();
        }

        fn setup_expressions(&self) {
            let obj = &*self.obj();

            <Self as ObjectSubclass>::Type::this_expression("model")
                .chain_property::<model::ClientStateAuthPhoneNumber>("country-list")
                .bind(&self.input.get(), "model", Some(obj));
        }
    }
}

glib::wrapper! {
    pub(crate) struct PhoneNumber(ObjectSubclass<imp::PhoneNumber>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::ClientStateAuthPhoneNumber> for PhoneNumber {
    fn from(model: &model::ClientStateAuthPhoneNumber) -> Self {
        glib::Object::builder().property("model", model).build()
    }
}

impl PhoneNumber {
    pub(crate) fn model_(&self) -> model::ClientStateAuthPhoneNumber {
        self.model().unwrap()
    }

    pub(crate) async fn exit(&self) {
        self.cancel();
        self.log_out(&self.model_().auth_().client_()).await;
    }

    pub(crate) async fn log_out(&self, client: &model::Client) {
        if let Err(e) = client.log_out().await {
            utils::show_toast(
                self,
                gettext_f("Failed to log out: {error}", &[("error", &e.message)]),
            );
        }
    }

    pub(crate) async fn next(&self) {
        let imp = self.imp();

        let model = self.model_();
        let client = model.auth_().client_();

        self.freeze(false, true);

        let abort_registration = self.setup_abort_handle();
        let result = future::Abortable::new(
            model.send_phone_number(imp.input.text().as_str()),
            abort_registration,
        )
        .await;

        if let Ok(result) = result {
            match result {
                model::SendPhoneNumberResult::AlreadyLoggedIn(client_session) => {
                    utils::ancestor::<_, ui::ClientManagerView>(self)
                        .set_active_client(&client_session.client_());

                    self.log_out(&client).await;
                }
                model::SendPhoneNumberResult::Err(e) => {
                    utils::show_toast(
                        self,
                        gettext_f(
                            "Failed to use phone number: {error}",
                            &[("error", &e.message)],
                        ),
                    );

                    self.focus_input();
                }
                model::SendPhoneNumberResult::Ok => {}
            }
        }

        self.freeze(false, false);
    }

    pub(crate) async fn request_qr_code(&self) {
        self.freeze(true, true);

        let abort_registration = self.setup_abort_handle();
        if let Ok(Err(e)) =
            future::Abortable::new(self.model_().request_qr_code(), abort_registration).await
        {
            utils::show_toast(
                self,
                gettext_f(
                    "Failed to request QR code: {error}",
                    &[("error", &e.message)],
                ),
            );
        }

        self.freeze(true, false);
    }
    pub(crate) fn focus_input(&self) {
        glib::idle_add_local_once(clone!(
            #[weak(rename_to = obj)]
            self,
            move || {
                obj.imp().input.select_number_without_calling_code();
            }
        ));
    }

    fn freeze(&self, qr: bool, freeze: bool) {
        let imp = self.imp();

        imp.input.set_sensitive(!freeze);
        imp.next_button_stack
            .set_visible_child_name(if !qr && freeze { "spinner" } else { "label" });
        imp.qr_code_spinner.set_spinning(qr && freeze);

        self.action_set_enabled("login.phone-number.next", !freeze);
        self.action_set_enabled("login.phone-number.use-qr-code", !qr || !freeze);
    }

    fn cancel(&self) {
        if let Some(handle) = &*self.imp().abort_handle.borrow() {
            handle.abort();
        }
    }

    fn setup_abort_handle(&self) -> future::AbortRegistration {
        let (abort_handle, abort_registration) = future::AbortHandle::new_pair();
        if let Some(handle) = self.imp().abort_handle.replace(Some(abort_handle)) {
            handle.abort();
        }

        abort_registration
    }
}
