use adw::prelude::*;
use gettextrs::gettext;
use glib::clone;
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
    #[template(resource = "/app/drey/paper-plane/ui/login/registration.ui")]
    #[properties(wrapper_type = super::Registration)]
    pub(crate) struct Registration {
        #[property(get, set)]
        pub(super) model: glib::WeakRef<model::ClientStateAuthRegistration>,
        #[template_child]
        pub(super) next_button_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) input_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) first_name_entry_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) last_name_entry_row: TemplateChild<adw::EntryRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Registration {
        const NAME: &'static str = "PaplLoginRegistration";
        type Type = super::Registration;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action_async(
                "login.registration.next",
                None,
                |widget, _, _| async move {
                    widget.next().await;
                },
            );

            klass.install_action(
                "login.registration.show-tos-dialog",
                None,
                move |widget, _, _| widget.show_tos_dialog(false),
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Registration {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }
        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }
        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn dispose(&self) {
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for Registration {
        fn root(&self) {
            self.parent_root();
            self.obj().focus_first_name_entry_row();
        }
    }

    #[gtk::template_callbacks]
    impl Registration {
        #[template_callback]
        fn on_first_name_entry_row_activated(&self) {
            self.obj()
                .activate_action("login.registration.next", None)
                .unwrap();
        }

        #[template_callback]
        fn on_last_name_entry_row_activated(&self) {
            self.obj()
                .activate_action("login.registration.next", None)
                .unwrap();
        }

        #[template_callback]
        fn on_tos_label_link_activated(&self) -> glib::Propagation {
            self.obj()
                .activate_action("login.registration.show-tos-dialog", None)
                .unwrap();
            glib::Propagation::Stop
        }
    }
}

glib::wrapper! {
    pub(crate) struct Registration(ObjectSubclass<imp::Registration>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::ClientStateAuthRegistration> for Registration {
    fn from(model: &model::ClientStateAuthRegistration) -> Self {
        glib::Object::builder().property("model", model).build()
    }
}

impl Registration {
    pub(crate) fn model_(&self) -> model::ClientStateAuthRegistration {
        self.model().unwrap()
    }

    pub(crate) async fn next(&self) {
        self.freeze(true);

        let imp = self.imp();

        let result = self
            .model_()
            .send_registration(
                imp.first_name_entry_row.text().into(),
                imp.last_name_entry_row.text().into(),
            )
            .await;
        if let Err(e) = result {
            utils::show_toast(
                self,
                gettext_f(
                    "Failed to register account: {error}",
                    &[("error", &e.message)],
                ),
            );

            self.focus_first_name_entry_row();
        }

        self.freeze(false);
    }

    fn show_tos_dialog(&self, user_needs_to_accept: bool) {
        let model = self.model_();

        let dialog = adw::AlertDialog::builder()
            .body_use_markup(true)
            .body(utils::parse_formatted_text(
                model.data().0.terms_of_service.text,
            ))
            .build();

        if user_needs_to_accept {
            dialog.set_heading(Some(&gettext("Do you accept the Terms of Service?")));
            dialog.add_responses(&[("no", &gettext("_No")), ("yes", &gettext("_Yes"))]);
            dialog.set_default_response(Some("no"));
        } else {
            dialog.set_heading(Some(&gettext("Terms of Service")));
            dialog.add_response("ok", &gettext("_OK"));
            dialog.set_default_response(Some("ok"));
        }

        dialog.choose(
            self.root().unwrap().downcast_ref::<gtk::Window>().unwrap(),
            gio::Cancellable::NONE,
            clone!(
                #[weak(rename_to = obj)]
                self,
                move |response| {
                    if response == "no" {
                        // If the user declines the ToS, don't proceed and just stay in
                        // the view but unfreeze it again.
                        obj.freeze(false);
                    } else if response == "yes" {
                        // User has accepted the ToS, so we can proceed in the login
                        // flow.
                        utils::spawn(clone!(
                            #[weak]
                            obj,
                            async move {
                                obj.next().await;
                            }
                        ));
                    }
                }
            ),
        );
    }

    pub(crate) fn focus_first_name_entry_row(&self) {
        glib::idle_add_local_once(clone!(
            #[weak(rename_to = obj)]
            self,
            move || {
                obj.imp().first_name_entry_row.grab_focus();
            }
        ));
    }

    fn freeze(&self, freeze: bool) {
        let imp = self.imp();

        imp.input_box.set_sensitive(!freeze);
        imp.next_button_stack
            .set_visible_child_name(if freeze { "spinner" } else { "label" });

        self.action_set_enabled("login.registration.next", !freeze);
        self.action_set_enabled("login.resend-auth-code", !freeze);
    }
}
