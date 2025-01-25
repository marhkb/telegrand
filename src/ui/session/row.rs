use adw::prelude::*;
use gettextrs::gettext;
use glib::closure;
use glib::subclass::InitializingObject;
use glib::Properties;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::expressions;
use crate::i18n::gettext_f;
use crate::model;
use crate::strings;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::Row)]
    #[template(resource = "/app/drey/paper-plane/ui/session/row.ui")]
    pub(crate) struct Row {
        #[property(get, set)]
        pub(super) session: glib::WeakRef<model::ClientStateSession>,
        #[template_child]
        pub(super) display_name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) username_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) unread_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) spinner: TemplateChild<gtk::Spinner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "PaplSessionRow";
        type Type = super::Row;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action_async(
                "session-row.log-out-client",
                None,
                |widget, _, _| async move {
                    widget.log_out().await;
                },
            );
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Row {
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
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for Row {}
}

glib::wrapper! {
    pub(crate) struct Row(ObjectSubclass<imp::Row>)
        @extends gtk::Widget, @implements gtk::Accessible;
}

impl Row {
    pub(crate) fn new(session: &model::ClientStateSession) -> Self {
        glib::Object::builder().property("session", session).build()
    }

    pub(crate) async fn log_out(&self) {
        if let Some(session) = self.session() {
            let dialog = adw::AlertDialog::builder()
                .heading_use_markup(true)
                .heading(gettext_f(
                    "Log out <i>{display_name}</i>",
                    &[(
                        "display_name",
                        &strings::user_display_name(&session.me_(), true),
                    )],
                ))
                .body(gettext("Are you sure you want to log out?"))
                .build();

            dialog.add_responses(&[
                ("cancel", &gettext("_Cancel")),
                ("log-out", &gettext("_Log out")),
            ]);
            dialog.set_default_response(Some("cancel"));
            dialog.set_response_appearance("log-out", adw::ResponseAppearance::Destructive);

            if dialog
                .choose_future(self.root().unwrap().downcast_ref::<gtk::Window>().unwrap())
                .await
                == "log-out"
            {
                let imp = self.imp();

                imp.stack.set_visible_child(&imp.spinner.get());
                imp.spinner.set_spinning(true);

                self.action_set_enabled("session-row.log-out-client", false);
                if let Err(e) = session.client_().log_out().await {
                    utils::show_toast(
                        self,
                        gettext_f("Failed to log out: {error}", &[("error", &e.message)]),
                    );
                    self.action_set_enabled("session-row.log-out-client", true);
                }

                imp.stack.set_visible_child_name("button");
                imp.spinner.set_spinning(false);
            }
        }
    }

    fn setup_expressions(&self) {
        let imp = self.imp();
        let me_expr =
            Self::this_expression("session").chain_property::<model::ClientStateSession>("me");

        // Bind the name
        expressions::user_display_name(&me_expr).bind(
            &*imp.display_name_label,
            "label",
            Some(self),
        );

        // Bind the username
        let username_expr = me_expr.chain_property::<model::User>("username");
        username_expr
            .chain_closure::<String>(closure!(|_: Self, username: String| {
                format!("@{username}")
            }))
            .bind(&*imp.username_label, "label", Some(self));

        let username_not_empty_expr =
            username_expr.chain_closure::<bool>(closure!(|_: Self, username: String| {
                !username.is_empty()
            }));

        username_not_empty_expr
            .chain_closure::<f64>(closure!(|_: Self, not_empty: bool| {
                if not_empty {
                    1.0
                } else {
                    0.5
                }
            }))
            .bind(&*imp.display_name_label, "yalign", Some(self));

        username_not_empty_expr.bind(&*imp.username_label, "visible", Some(self));
    }
}
