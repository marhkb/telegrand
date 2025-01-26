mod code;
mod other_device;
mod password;
mod phone_number;
mod registration;

use adw::prelude::*;
use gettextrs::gettext;
use glib::subclass::InitializingObject;
use glib::Properties;
use gtk::gio;
use gtk::glib;
use gtk::glib::clone;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

pub(crate) use self::code::Code;
pub(crate) use self::other_device::OtherDevice;
pub(crate) use self::password::Password;
pub(crate) use self::phone_number::PhoneNumber;
pub(crate) use self::registration::Registration;
use crate::model;
use crate::ui;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/app/drey/paper-plane/ui/login/mod.ui")]
    #[properties(wrapper_type = super::Login)]
    pub(crate) struct Login {
        #[property(get, set = Self::set_model, construct, explicit_notify)]
        pub(super) model: glib::WeakRef<model::ClientStateAuth>,
        #[template_child]
        pub(super) animated_bin: TemplateChild<ui::AnimatedBin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Login {
        const NAME: &'static str = "PaplLogin";
        type Type = super::Login;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("login.reset", None, |widget, _, _| {
                widget.reset();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Login {
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
            self.obj().first_child().unwrap().unparent();
        }
    }

    impl WidgetImpl for Login {}

    impl Login {
        fn set_model(&self, model: &model::ClientStateAuth) {
            let obj = &*self.obj();
            if obj.model().as_ref() == Some(model) {
                return;
            }

            if let Some(state) = model.state() {
                obj.update_state(state);
            }
            model.connect_state_notify(clone!(
                #[weak]
                obj,
                move |auth| {
                    if let Some(state) = auth.state() {
                        obj.update_state(state);
                    }
                }
            ));

            self.model.set(Some(model));
            obj.notify_model();
        }
    }
}

glib::wrapper! {
    pub(crate) struct Login(ObjectSubclass<imp::Login>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl From<&model::ClientStateAuth> for Login {
    fn from(model: &model::ClientStateAuth) -> Self {
        glib::Object::builder().property("model", model).build()
    }
}

impl Login {
    pub(crate) fn model_(&self) -> model::ClientStateAuth {
        self.model().unwrap()
    }

    pub(crate) fn reset(&self) {
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Reset Registration Process?"))
            .body(gettext("If you reset the registration process, the previous progress will be irrevocably lost."))
            .build();

        dialog.add_responses(&[
            ("cancel", &gettext("_Cancel")),
            ("reset", &gettext("_Reset")),
        ]);
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);

        let model = self.model_();

        dialog.choose(
            self.root().unwrap().downcast_ref::<gtk::Window>().unwrap(),
            gio::Cancellable::NONE,
            clone!(
                #[weak]
                model,
                move |response| {
                    if response == "reset" {
                        model.reset();
                    }
                }
            ),
        );
    }

    fn update_state(&self, state: glib::Object) {
        let animated_bin = &self.imp().animated_bin.get();

        if let Some(state) = state.downcast_ref::<model::ClientStateAuthPhoneNumber>() {
            match animated_bin.child().and_downcast::<PhoneNumber>() {
                Some(phone_number) => phone_number.set_model(state),
                None => animated_bin.set_child(&PhoneNumber::from(state)),
            }
        } else if let Some(state) = state.downcast_ref::<model::ClientStateAuthOtherDevice>() {
            match animated_bin.child().and_downcast::<OtherDevice>() {
                Some(other_device) => other_device.set_model(state),
                None => animated_bin.set_child(&OtherDevice::from(state)),
            }
        } else if let Some(state) = state.downcast_ref::<model::ClientStateAuthCode>() {
            match animated_bin.child().and_downcast::<Code>() {
                Some(code) => code.set_model(state),
                None => animated_bin.set_child(&Code::from(state)),
            }
        } else if let Some(state) = state.downcast_ref::<model::ClientStateAuthPassword>() {
            match animated_bin.child().and_downcast::<Password>() {
                Some(password) => password.set_model(state),
                None => animated_bin.set_child(&Password::from(state)),
            }
        } else if let Some(state) = state.downcast_ref::<model::ClientStateAuthRegistration>() {
            match animated_bin.child().and_downcast::<Registration>() {
                Some(registration) => registration.set_model(state),
                None => animated_bin.set_child(&Registration::from(state)),
            }
        } else {
            unreachable!()
        }
    }
}
