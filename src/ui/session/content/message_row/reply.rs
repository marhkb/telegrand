use std::cell::RefCell;

use gettextrs::gettext;
use glib::clone;
use glib::ParamSpec;
use glib::Properties;
use glib::Value;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::strings;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::MessageReply)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/message_row/reply.ui")]
    pub(crate) struct MessageReply {
        pub(super) sender_color_class: RefCell<Option<String>>,
        pub(super) bindings: RefCell<Vec<gtk::ExpressionWatch>>,

        #[property(get, set, construct_only)]
        pub(super) message: glib::WeakRef<model::Message>,

        #[template_child]
        pub(super) separator: TemplateChild<gtk::Separator>,
        #[template_child]
        pub(super) labels_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) sender_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) message_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageReply {
        const NAME: &'static str = "PaplMessageReply";
        type Type = super::MessageReply;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("messagereply");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageReply {
        fn properties() -> &'static [ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &Value, pspec: &ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &ParamSpec) -> Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.message_label.set_label(&gettext("Loading…"));

            let obj = self.obj();
            utils::spawn(clone!(
                #[weak]
                obj,
                async move {
                    obj.load_replied_message().await;
                }
            ));
        }

        fn dispose(&self) {
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for MessageReply {}
}

glib::wrapper! {
    pub(crate) struct MessageReply(ObjectSubclass<imp::MessageReply>)
        @extends gtk::Widget;
}

impl MessageReply {
    pub(crate) fn new(message: &model::Message) -> Self {
        glib::Object::builder().property("message", message).build()
    }

    async fn load_replied_message(&self) {
        let imp = self.imp();

        let message = self.message().unwrap();
        let is_outgoing = message.is_outgoing();

        match message.reply_to().unwrap().0 {
            tdlib::enums::MessageReplyTo::Message(reply_to) => {
                let chat = if reply_to.chat_id != 0 {
                    message.chat_().session_().chat(reply_to.chat_id)
                } else {
                    message.chat_()
                };

                match chat.fetch_message(reply_to.message_id).await {
                    Ok(message) => self.update_from_message(&message, is_outgoing),
                    Err(_) => imp.message_label.set_label("Deleted message"),
                };
            }
            tdlib::enums::MessageReplyTo::Story(_) => {
                // TODO: Implement story replies
                unimplemented!()
            }
        };
    }

    pub(crate) fn set_max_char_width(&self, n_chars: i32) {
        self.imp().message_label.set_max_width_chars(n_chars);
        self.imp().sender_label.set_max_width_chars(n_chars);
    }

    fn update_from_message(&self, replied_message: &model::Message, is_outgoing: bool) {
        let imp = self.imp();
        let mut bindings = imp.bindings.borrow_mut();
        while let Some(binding) = bindings.pop() {
            binding.unwatch();
        }

        // Remove the previous color css class
        let mut sender_color_class = imp.sender_color_class.borrow_mut();
        if let Some(class) = sender_color_class.as_ref() {
            self.remove_css_class(class);
        }
        // Show sender label, if needed
        let show_sender = !matches!(
            replied_message.chat_().chat_type(),
            model::ChatType::Supergroup(data) if data.is_channel()
        );
        if show_sender {
            let sender_name_expression = replied_message.sender_name_expression();
            let sender_binding =
                sender_name_expression.bind(&*imp.sender_label, "label", glib::Object::NONE);

            bindings.push(sender_binding);

            if !is_outgoing {
                // Color sender label
                if let model::MessageSender::User(user) = replied_message.sender() {
                    let classes = &[
                        "sender-text-red",
                        "sender-text-orange",
                        "sender-text-violet",
                        "sender-text-green",
                        "sender-text-cyan",
                        "sender-text-blue",
                        "sender-text-pink",
                    ];

                    let color_class = classes[user.id() as usize % classes.len()];
                    self.add_css_class(color_class);

                    *sender_color_class = Some(color_class.into());
                }
            }
            imp.sender_label.set_visible(true);
        }

        // Set content label expression

        let caption = strings::message_content(replied_message.clone().as_ref());
        imp.message_label.set_label(&caption);
    }
}
