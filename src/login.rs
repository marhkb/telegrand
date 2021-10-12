use gettextrs::gettext;
use glib::clone;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use locale_config::Locale;
use tdgrand::{enums::AuthorizationState, functions, types};

use crate::config;
use crate::utils::{do_async, parse_formatted_text};

mod imp {
    use super::*;
    use adw::subclass::prelude::BinImpl;
    use glib::subclass::Signal;
    use gtk::{gio, CompositeTemplate};
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/login.ui")]
    pub struct Login {
        pub client_id: Cell<i32>,
        pub tos_text: RefCell<String>,
        pub show_tos_popup: Cell<bool>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub previous_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub next_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub next_spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub content: TemplateChild<adw::Leaflet>,
        #[template_child]
        pub phone_number_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub welcome_page_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub use_test_dc_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub code_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub code_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub registration_first_name_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub registration_last_name_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub registration_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub tos_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub password_entry: TemplateChild<gtk::PasswordEntry>,
        #[template_child]
        pub password_error_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Login {
        const NAME: &'static str = "Login";
        type Type = super::Login;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.install_action("login.previous", None, move |widget, _, _| {
                widget.previous()
            });
            klass.install_action("login.next", None, move |widget, _, _| widget.next());
            klass.install_action("tos.dialog", None, move |widget, _, _| {
                widget.show_tos_dialog(false)
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Login {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("new-session", &[], <()>::static_type().into()).build()]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Show the previous button on all pages except in the "phone-number-page"
            let self_ = imp::Login::from_instance(obj);
            let previous_button = &*self_.previous_button;
            self_.content.connect_visible_child_name_notify(
                clone!(@weak previous_button => move |content| {
                    let visible_page = content.visible_child_name().unwrap();
                    if visible_page == "phone-number-page" {
                        previous_button.set_visible(false);
                    } else {
                        previous_button.set_visible(true);
                    }
                }),
            );

            // Bind the use-test-dc setting to the relative switch
            let use_test_dc_switch = &*self_.use_test_dc_switch;
            let settings = gio::Settings::new(config::APP_ID);
            settings
                .bind("use-test-dc", use_test_dc_switch, "state")
                .build();

            self_.tos_label.connect_activate_link(|label, _| {
                label.activate_action("tos.dialog", None);
                gtk::Inhibit(true)
            });
        }
    }

    impl WidgetImpl for Login {}
    impl BinImpl for Login {}
}

glib::wrapper! {
    pub struct Login(ObjectSubclass<imp::Login>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for Login {
    fn default() -> Self {
        Self::new()
    }
}

impl Login {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Login")
    }

    pub fn login_client(&self, client_id: i32) {
        let self_ = imp::Login::from_instance(self);
        self_.client_id.set(client_id);

        // We don't know what login page to show at this point, so we show an empty page until we
        // receive an AuthenticationState that will eventually show the related login page.
        self_.main_stack.set_visible_child_name("empty-page");

        self_.phone_number_entry.set_text("");
        self_.registration_first_name_entry.set_text("");
        self_.registration_last_name_entry.set_text("");
        self_.code_entry.set_text("");
        self_.password_entry.set_text("");

        self.unfreeze();
        self.action_set_enabled("login.next", false);
    }

    pub fn set_authorization_state(&self, state: AuthorizationState) {
        let self_ = imp::Login::from_instance(self);

        match state {
            AuthorizationState::WaitTdlibParameters => {
                self.send_tdlib_parameters();
            }
            AuthorizationState::WaitEncryptionKey(_) => {
                self.send_encryption_key();
            }
            AuthorizationState::WaitPhoneNumber => {
                self.set_visible_page_name(
                    "phone-number-page",
                    [&*self_.phone_number_entry],
                    &self_.welcome_page_error_label,
                    &*self_.phone_number_entry,
                );
            }
            AuthorizationState::WaitCode(_) => {
                self.set_visible_page_name(
                    "code-page",
                    [&*self_.code_entry],
                    &self_.code_error_label,
                    &*self_.code_entry,
                );
            }
            AuthorizationState::WaitOtherDeviceConfirmation(_) => {
                todo!()
            }
            AuthorizationState::WaitRegistration(data) => {
                self_.show_tos_popup.set(data.terms_of_service.show_popup);
                self_
                    .tos_text
                    .replace(parse_formatted_text(data.terms_of_service.text));

                self.set_visible_page_name(
                    "registration-page",
                    [
                        &*self_.registration_first_name_entry,
                        &*self_.registration_last_name_entry,
                    ],
                    &self_.registration_error_label,
                    &*self_.registration_first_name_entry,
                );
            }
            AuthorizationState::WaitPassword(_) => {
                // When we enter the password page, the password to be entered should be masked by
                // default, so the peek icon is turned off and on again.
                self_.password_entry.set_show_peek_icon(false);
                self_.password_entry.set_show_peek_icon(true);

                self.set_visible_page_name(
                    "password-page",
                    [&*self_.password_entry],
                    &self_.password_error_label,
                    &*self_.password_entry,
                );
            }
            AuthorizationState::Ready => {
                self.emit_by_name("new-session", &[]).unwrap();
            }
            _ => {}
        }
    }

    fn set_visible_page_name<'a, W, E, I>(
        &self,
        page_name: &str,
        editables_to_clear: I,
        error_label_to_clear: &gtk::Label,
        widget_to_focus: &W,
    ) where
        W: WidgetExt,
        E: EditableExt,
        I: IntoIterator<Item = &'a E>,
    {
        let self_ = imp::Login::from_instance(self);

        // Before transition to the page, be sure to reset the error label because it still might
        // conatain an error message from the time when it was previously visited.
        error_label_to_clear.set_label("");
        // Also clear all editables on that page.
        editables_to_clear
            .into_iter()
            .for_each(|editable| editable.set_text(""));

        self_.content.set_visible_child_name(page_name);

        // After we've transitioned to a new login page, let's be sure that we set the stack here
        // to an ancestor widget of the login leaflet because we might still be in the empty page.
        self_.main_stack.set_visible_child_name("login-flow-page");

        self.unfreeze();
        widget_to_focus.grab_focus();
    }

    fn previous(&self) {
        let self_ = imp::Login::from_instance(self);
        self_.content.set_visible_child_name("phone-number-page");

        // Grab focus for entry after reset.
        self_.phone_number_entry.grab_focus();
    }

    fn next(&self) {
        self.freeze();

        let self_ = imp::Login::from_instance(self);
        let visible_page = self_.content.visible_child_name().unwrap();

        match visible_page.as_str() {
            "phone-number-page" => self.send_phone_number(),

            "code-page" => self.send_code(),
            "registration-page" => {
                if self_.show_tos_popup.get() {
                    // Force the ToS dialog for the user before he can proceed
                    self.show_tos_dialog(true);
                } else {
                    // Just proceed if the user either doesn't need to accept the ToS
                    self.send_registration()
                }
            }
            "password-page" => self.send_password(),
            other => unreachable!("no page named '{}'", other),
        }
    }

    fn show_tos_dialog(&self, user_needs_to_accept: bool) {
        let self_ = imp::Login::from_instance(self);

        let builder = gtk::MessageDialog::builder()
            .use_markup(true)
            .secondary_text(&*self_.tos_text.borrow())
            .modal(true)
            .transient_for(self.root().unwrap().downcast_ref::<gtk::Window>().unwrap());

        let dialog = if user_needs_to_accept {
            builder
                .buttons(gtk::ButtonsType::YesNo)
                .text(&gettext("Do You Accept the Terms of Service?"))
        } else {
            builder
                .buttons(gtk::ButtonsType::Ok)
                .text(&gettext("Terms of Service"))
        }
        .build();

        dialog.run_async(clone!(@weak self as obj => move |dialog, response| {
            if matches!(response, gtk::ResponseType::No) {
                // If the user declines the ToS, don't proceed and just stay in
                // the view but unfreeze it again.
                obj.unfreeze();
            } else if matches!(response, gtk::ResponseType::Yes) {
                // User has accepted the ToS, so we can proceed in the login
                // flow.
                obj.send_registration();
            }
            dialog.close();
        }));
    }

    fn freeze(&self) {
        self.action_set_enabled("login.previous", false);
        self.action_set_enabled("login.next", false);

        let self_ = imp::Login::from_instance(self);
        self_
            .next_stack
            .set_visible_child(&self_.next_spinner.get());
        self_.content.set_sensitive(false);
    }

    fn unfreeze(&self) {
        self.action_set_enabled("login.previous", true);
        self.action_set_enabled("login.next", true);

        let self_ = imp::Login::from_instance(self);
        self_.next_stack.set_visible_child(&self_.next_label.get());
        self_.content.set_sensitive(true);
    }

    fn send_tdlib_parameters(&self) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let use_test_dc = self_.use_test_dc_switch.state();
        let database_directory =
            format!("{}/telegrand/db0", glib::user_data_dir().to_str().unwrap());
        let parameters = types::TdlibParameters {
            use_test_dc,
            database_directory,
            use_message_database: true,
            use_secret_chats: true,
            api_id: config::TG_API_ID,
            api_hash: config::TG_API_HASH.to_string(),
            system_language_code: Locale::current().to_string(),
            device_model: "Desktop".to_string(),
            application_version: config::VERSION.to_string(),
            enable_storage_optimizer: true,
            ..types::TdlibParameters::default()
        };
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::SetTdlibParameters::new()
                    .parameters(parameters)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    show_error_label(
                        &imp::Login::from_instance(&obj).welcome_page_error_label,
                        &err.message
                    );
                }
            }),
        );
    }

    fn send_encryption_key(&self) {
        let self_ = imp::Login::from_instance(self);
        let client_id = self_.client_id.get();
        let encryption_key = "".to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::CheckDatabaseEncryptionKey::new()
                    .encryption_key(encryption_key)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    show_error_label(
                        &imp::Login::from_instance(&obj).welcome_page_error_label,
                        &err.message
                    )
                }
            }),
        );
    }

    fn send_phone_number(&self) {
        let self_ = imp::Login::from_instance(self);

        reset_error_label(&self_.welcome_page_error_label);

        let client_id = self_.client_id.get();
        let phone_number = self_.phone_number_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::SetAuthenticationPhoneNumber::new()
                    .phone_number(phone_number)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                let self_ = imp::Login::from_instance(&obj);
                obj.handle_user_result(
                    result,
                    &self_.welcome_page_error_label,
                    &*self_.phone_number_entry
                )
            }),
        );
    }

    fn send_code(&self) {
        let self_ = imp::Login::from_instance(self);

        reset_error_label(&self_.code_error_label);

        let client_id = self_.client_id.get();
        let code = self_.code_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::CheckAuthenticationCode::new()
                    .code(code)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                let self_ = imp::Login::from_instance(&obj);
                obj.handle_user_result(result, &self_.code_error_label, &*self_.code_entry)
            }),
        );
    }

    fn send_registration(&self) {
        let self_ = imp::Login::from_instance(self);

        reset_error_label(&self_.registration_error_label);

        let client_id = self_.client_id.get();
        let first_name = self_.registration_first_name_entry.text().to_string();
        let last_name = self_.registration_last_name_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::RegisterUser::new()
                    .first_name(first_name)
                    .last_name(last_name)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                let self_ = imp::Login::from_instance(&obj);
                obj.handle_user_result(
                    result,
                    &self_.registration_error_label,
                    &*self_.registration_first_name_entry
                )
            }),
        );
    }

    fn send_password(&self) {
        let self_ = imp::Login::from_instance(self);

        reset_error_label(&self_.password_error_label);

        let client_id = self_.client_id.get();
        let password = self_.password_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::CheckAuthenticationPassword::new()
                    .password(password)
                    .send(client_id)
                    .await
            },
            clone!(@weak self as obj => move |result| async move {
                let self_ = imp::Login::from_instance(&obj);
                obj.handle_user_result(
                    result,
                    &self_.password_error_label,
                    &*self_.password_entry
                )
            }),
        );
    }

    fn handle_user_result<T, W>(
        &self,
        result: Result<T, types::Error>,
        error_label: &gtk::Label,
        widget_to_focus: &W,
    ) where
        W: WidgetExt,
    {
        if let Err(err) = result {
            show_error_label(error_label, &err.message);
            self.unfreeze();
            // Grab focus for entry again after error.
            widget_to_focus.grab_focus();
        }
    }

    pub fn client_id(&self) -> i32 {
        let self_ = imp::Login::from_instance(self);
        self_.client_id.get()
    }

    pub fn connect_new_session<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("new-session", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);

            None
        })
        .unwrap()
    }
}

fn show_error_label(error_label: &gtk::Label, message: &str) {
    error_label.set_text(message);
    error_label.set_visible(true);
}

fn reset_error_label(error_label: &gtk::Label) {
    error_label.set_text("");
    error_label.set_visible(false);
}
