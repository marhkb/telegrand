use gettextrs::gettext;
use glib::clone;
use glib::closure;
use glib::subclass::InitializingObject;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::i18n::gettext_f;
use crate::model;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/app/drey/paper-plane/ui/login/code.ui")]
    #[properties(wrapper_type = super::Code)]
    pub(crate) struct Code {
        #[property(get, set)]
        pub(super) model: glib::WeakRef<model::ClientStateAuthCode>,
        #[template_child]
        pub(super) status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) input_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) entry_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) next_button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) resend_link_button: TemplateChild<gtk::LinkButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Code {
        const NAME: &'static str = "PaplLoginCode";
        type Type = super::Code;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action_async("login.code.next", None, |widget, _, _| async move {
                widget.next().await;
            });

            klass.install_action_async(
                "login.code.resend-auth-code",
                None,
                |widget, _, _| async move {
                    widget.resend_auth_code().await;
                },
            );
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Code {
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
            self.obj().first_child().unwrap().unparent();
        }
    }

    impl WidgetImpl for Code {
        fn root(&self) {
            self.parent_root();
            self.obj().focus_entry_row();
        }
    }

    #[gtk::template_callbacks]
    impl Code {
        #[template_callback]
        fn on_entry_row_activated(&self) {
            self.obj().activate_action("login.code.next", None).unwrap();
        }

        fn setup_expressions(&self) {
            let obj = &*self.obj();

            let model_expr = <Self as ObjectSubclass>::Type::this_expression("model");
            let data_expr = model_expr.chain_property::<model::ClientStateAuthCode>("data");
            let countdown_expr =
                model_expr.chain_property::<model::ClientStateAuthCode>("countdown");

            gtk::ClosureExpression::new::<String>(
                [data_expr.as_ref(), countdown_expr.as_ref()],
                closure!(
                    |_: <Self as ObjectSubclass>::Type, data: model::BoxedAuthorizationStateWaitCode, countdown: i32| {
                        data.0
                            .code_info
                            .next_type
                            .map(|type_| {
                                if countdown > 0 {
                                    gettext_f(
                                        "Send code via {type} (may still arrive within {countdown} seconds)",
                                        &[("type", &stringify_auth_code_type(type_)), ("countdown", &countdown.to_string())],
                                    )
                                } else {
                                    gettext_f(
                                        "Send code via {type}",
                                        &[("type", &stringify_auth_code_type(type_))],
                                    )
                                }
                            })
                            .unwrap_or_default()
                    }
                ),
            )
            .bind(&self.resend_link_button.get(), "label", Some(obj));

            data_expr
                .chain_closure::<bool>(closure!(
                    |_: <Self as ObjectSubclass>::Type,
                     data: model::BoxedAuthorizationStateWaitCode| {
                        data.0.code_info.next_type.is_some()
                    }
                ))
                .bind(&self.resend_link_button.get(), "visible", Some(obj));

            data_expr
                .chain_closure::<String>(closure!(
                    |_: <Self as ObjectSubclass>::Type,
                     data: model::BoxedAuthorizationStateWaitCode| {
                        gettext_f(
                            "The code will arrive to you via {type}.",
                            &[("type", &stringify_auth_code_type(data.0.code_info.r#type))],
                        )
                    }
                ))
                .bind(&self.status_page.get(), "description", Some(obj));
        }
    }
}

glib::wrapper! {
    pub(crate) struct Code(ObjectSubclass<imp::Code>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::ClientStateAuthCode> for Code {
    fn from(model: &model::ClientStateAuthCode) -> Self {
        glib::Object::builder().property("model", model).build()
    }
}

impl Code {
    pub(crate) fn model_(&self) -> model::ClientStateAuthCode {
        self.model().unwrap()
    }

    pub(crate) async fn next(&self) {
        self.freeze(true);

        if let Err(e) = self
            .model_()
            .send_code(self.imp().entry_row.text().into())
            .await
        {
            utils::show_toast(
                self,
                gettext_f("Failed to authenticate: {error}", &[("error", &e.message)]),
            );

            self.focus_entry_row();
        }

        self.freeze(false);
    }

    pub(crate) async fn resend_auth_code(&self) {
        if let Err(e) = self.model_().resend_auth_code().await {
            utils::show_toast(
                self,
                gettext_f(
                    "Failed to resend auth code: {error}",
                    &[("error", &e.message)],
                ),
            );
        }
    }

    pub(crate) fn focus_entry_row(&self) {
        glib::idle_add_local_once(clone!(
            #[weak(rename_to = obj)]
            self,
            move || {
                obj.imp().entry_row.grab_focus();
            }
        ));
    }

    fn freeze(&self, freeze: bool) {
        let imp = self.imp();

        imp.input_box.set_sensitive(!freeze);
        imp.next_button_stack
            .set_visible_child_name(if freeze { "spinner" } else { "label" });

        self.action_set_enabled("login.code.next", !freeze);
        self.action_set_enabled("login.code.resend-auth-code", !freeze);
    }
}

fn stringify_auth_code_type(code_type: tdlib::enums::AuthenticationCodeType) -> String {
    use tdlib::enums::AuthenticationCodeType::*;

    match code_type {
        // Translators: This is an authentication method
        TelegramMessage(_) => gettext("Telegram"),
        // Translators: This is an authentication method
        Sms(_) | FirebaseAndroid(_) | FirebaseIos(_) => gettext("SMS"),
        // Translators: This is an authentication method
        Call(_) => gettext("Call"),
        // Translators: This is an authentication method
        FlashCall(_) => gettext("Flash Call"),
        // Translators: This is an authentication method
        MissedCall(_) => gettext("Missed Call"),
        // Translators: This is an authentication method
        Fragment(_) => gettext("Fragment"),
    }
}
