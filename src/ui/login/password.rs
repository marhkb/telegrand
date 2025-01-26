use adw::prelude::*;
use gettextrs::gettext;
use glib::clone;
use glib::closure;
use glib::subclass::InitializingObject;
use glib::Properties;
use gtk::gio;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::i18n::gettext_f;
use crate::model;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/app/drey/paper-plane/ui/login/password.ui")]
    #[properties(wrapper_type = super::Password)]
    pub(crate) struct Password {
        #[property(get, set)]
        pub(super) model: glib::WeakRef<model::ClientStateAuthPassword>,

        #[template_child]
        pub(super) navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub(super) password_input_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) password_entry_row: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub(super) password_hint_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) password_hint_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) next_button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) password_forgotten_link_button: TemplateChild<gtk::LinkButton>,

        #[template_child]
        pub(super) password_forgotten_input_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) send_code_button_stack: TemplateChild<gtk::Stack>,

        #[template_child]
        pub(super) send_recovery_code_input_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) password_recovery_code_entry_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) send_recovery_code_button_stack: TemplateChild<gtk::Stack>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Password {
        const NAME: &'static str = "PaplLoginPassword";
        type Type = super::Password;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action_async("login.password.next", None, |widget, _, _| async move {
                widget.next().await;
            });

            klass.install_action("login.password.forgot", None, |widget, _, _| {
                widget.forgot_password();
            });

            klass.install_action_async(
                "login.password.start-recovery",
                None,
                |widget, _, _| async move {
                    widget.start_recovery().await;
                },
            );

            klass.install_action(
                "login.password.delete-account",
                None,
                move |widget, _, _| {
                    widget.delete_account();
                },
            );

            klass.install_action_async(
                "login.password.send-recovery-code",
                None,
                move |widget, _, _| async move {
                    widget.send_password_recovery_code().await;
                },
            );

            klass.install_action(
                "login.password.show-no-email-access-dialog",
                None,
                |widget, _, _| {
                    widget.show_no_email_access_dialog();
                },
            );
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Password {
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
            self.obj().setup_expressions();
        }

        fn dispose(&self) {
            self.obj().first_child().unwrap().unparent();
        }
    }

    impl WidgetImpl for Password {
        fn root(&self) {
            self.parent_root();
            self.obj().focus_password_entry_row();
        }
    }

    #[gtk::template_callbacks]
    impl Password {
        #[template_callback]
        fn on_password_entry_row_activated(&self) {
            self.obj()
                .activate_action("login.password.next", None)
                .unwrap();
        }
    }
}

glib::wrapper! {
    pub(crate) struct Password(ObjectSubclass<imp::Password>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::ClientStateAuthPassword> for Password {
    fn from(model: &model::ClientStateAuthPassword) -> Self {
        glib::Object::builder().property("model", model).build()
    }
}

impl Password {
    pub(crate) fn model_(&self) -> model::ClientStateAuthPassword {
        self.model().unwrap()
    }

    fn setup_expressions(&self) {
        let imp = self.imp();

        let data_expr =
            Self::this_expression("model").chain_property::<model::ClientStateAuthPassword>("data");

        data_expr
            .chain_closure::<bool>(closure!(
                |_: Self, data: model::BoxedAuthorizationStateWaitPassword| {
                    !data.0.password_hint.is_empty()
                }
            ))
            .bind(&*imp.password_hint_row, "visible", Some(self));

        data_expr
            .chain_closure::<String>(closure!(
                |_: Self, data: model::BoxedAuthorizationStateWaitPassword| data.0.password_hint
            ))
            .bind(&*imp.password_hint_label, "label", Some(self));
    }

    pub(crate) async fn next(&self) {
        self.freeze(true);

        if let Err(e) = self
            .model_()
            .send_password(self.imp().password_entry_row.text().into())
            .await
        {
            utils::show_toast(
                self,
                gettext_f(
                    "Failed to verify password: {error}",
                    &[("error", &e.message)],
                ),
            );

            self.focus_password_entry_row();
        }

        self.freeze(false);
    }

    pub(crate) fn forgot_password(&self) {
        self.imp().navigation_view.push_by_tag("forget");
    }

    pub(crate) async fn start_recovery(&self) {
        self.freeze(true);

        match self.model_().recover_password().await {
            Ok(_) => self.imp().navigation_view.push_by_tag("recover"),
            Err(e) => {
                utils::show_toast(
                    self,
                    gettext_f(
                        "Failed to recover password: {error}",
                        &[("error", &e.message)],
                    ),
                );
            }
        }

        self.freeze(false);
    }

    pub(crate) fn delete_account(&self) {
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Warning"))
            .body(gettext(
                "You will lose all your chats and messages, along with any media and files you shared!\n\nDo you want to delete your account?",
            ))
            .build();

        dialog.add_responses(&[
            ("cancel", &gettext("_Cancel")),
            ("delete", &gettext("_Delete Account")),
        ]);
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

        dialog.choose(
            self.root().unwrap().downcast_ref::<gtk::Window>().unwrap(),
            gio::Cancellable::NONE,
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |response| {
                    if response == "delete" {
                        obj.freeze(true);
                        let client_id = obj.model().unwrap().auth().unwrap().client().unwrap().id();

                        utils::spawn(clone!(
                            #[weak]
                            obj,
                            async move {
                                let result = tdlib::functions::delete_account(
                                    "Cloud password lost and not recoverable".into(),
                                    String::new(),
                                    client_id,
                                )
                                .await;

                                // Just unfreeze in case of an error, else stay frozen until we are
                                // redirected to the welcome page.
                                if let Err(e) = result {
                                    log::error!("Failed to delete account: {e:?}");
                                    utils::show_toast(
                                        &obj,
                                        gettext_f(
                                            "Failed to delete account: {error}",
                                            &[("error", &e.message)],
                                        ),
                                    );

                                    obj.freeze(false);
                                }
                            }
                        ));
                    } else {
                        obj.imp().password_entry_row.grab_focus();
                    }
                }
            ),
        );
    }

    pub(crate) async fn send_password_recovery_code(&self) {
        use model::SendPasswordRecoveryCodeResult::*;

        let model = self.model_();

        match model
            .send_password_recovery_code(self.imp().password_recovery_code_entry_row.text().into())
            .await
        {
            Ok => {}
            Expired => match model.recover_password().await {
                Result::Ok(_) => {
                    utils::show_toast(self, gettext("Code expired. A new one has been sent you."))
                }
                Result::Err(e) => utils::show_toast(
                    self,
                    gettext_f(
                        "Code expired. Could not sent a new one: {error}",
                        &[("error", &e.message)],
                    ),
                ),
            },
            Err(e) => utils::show_toast(
                self,
                gettext_f(
                    "Failed to send password code: {error}",
                    &[("error", &e.message)],
                ),
            ),
        }
    }

    pub(crate) fn show_no_email_access_dialog(&self) {
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Sorry"))
            .body(gettext(
                "If you can't restore access to the e-mail, your remaining options are either to remember your password or to delete and then recreate your account.",
            ))
            .build();

        dialog.add_responses(&[("ok", &gettext("_OK"))]);
        dialog.set_default_response(Some("ok"));

        dialog.choose(
            self.root().unwrap().downcast_ref::<gtk::Window>().unwrap(),
            gio::Cancellable::NONE,
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |_| {
                    obj.imp().password_recovery_code_entry_row.grab_focus();
                }
            ),
        );
    }

    pub(crate) fn focus_password_entry_row(&self) {
        glib::idle_add_local_once(clone!(
            #[weak(rename_to = obj)]
            self,
            move || {
                obj.imp().password_entry_row.grab_focus();
            }
        ));
    }

    fn freeze(&self, freeze: bool) {
        let imp = self.imp();

        imp.password_input_box.set_sensitive(!freeze);
        imp.next_button_stack
            .set_visible_child_name(if freeze { "spinner" } else { "label" });

        imp.password_forgotten_input_box.set_sensitive(!freeze);
        imp.send_code_button_stack
            .set_visible_child_name(if freeze { "spinner" } else { "label" });

        imp.send_recovery_code_input_box.set_sensitive(!freeze);
        imp.send_recovery_code_button_stack
            .set_visible_child_name(if freeze { "spinner" } else { "label" });

        self.action_set_enabled("login.password.next", !freeze);
        self.action_set_enabled("login.password.forgot", !freeze);
        self.action_set_enabled("login.password.start-recovery", !freeze);
        self.action_set_enabled("login.password.delete-account", !freeze);
        self.action_set_enabled("login.password.send-recovery-code", !freeze);
        self.action_set_enabled("login.password.show-no-email-access-dialog", !freeze);
    }
}
