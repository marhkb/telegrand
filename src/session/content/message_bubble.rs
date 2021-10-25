use adw::{prelude::BinExt, subclass::prelude::BinImpl};
use gtk::{glib, pango, prelude::*, subclass::prelude::*, CompositeTemplate};
use tdgrand::enums::ChatType;

use crate::session::chat::{Message, MessageSender};
use crate::session::content::MessageLabel;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-bubble.ui")]
    pub struct MessageBubble {
        pub is_outgoing: Cell<bool>,
        pub sender_color_class: RefCell<Option<String>>,
        #[template_child]
        pub sender_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub message_label: TemplateChild<MessageLabel>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageBubble {
        const NAME: &'static str = "ContentMessageBubble";
        type Type = super::MessageBubble;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageBubble {}
    impl WidgetImpl for MessageBubble {}
    impl BinImpl for MessageBubble {}
}

glib::wrapper! {
    pub struct MessageBubble(ObjectSubclass<imp::MessageBubble>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for MessageBubble {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBubble {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MessageBubble")
    }

    pub fn set_message(&self, message: &Message) {
        let self_ = imp::MessageBubble::from_instance(self);

        // Remove previous css class
        if self_.is_outgoing.get() {
            self.remove_css_class("outgoing")
        }

        // Set new css class
        if message.is_outgoing() {
            self.add_css_class("outgoing");
        }

        self_.is_outgoing.set(message.is_outgoing());

        // Show sender label, if needed
        let show_sender = {
            if !message.is_outgoing() {
                matches!(
                    message.chat().type_(),
                    ChatType::BasicGroup(_) | ChatType::Supergroup(_)
                )
            } else {
                false
            }
        };
        if show_sender {
            let label = if let Some(Ok(label)) =
                self_.sender_bin.child().map(|w| w.downcast::<gtk::Label>())
            {
                label
            } else {
                let label = gtk::LabelBuilder::new()
                    .css_classes(vec!["sender-text".to_string()])
                    .halign(gtk::Align::Start)
                    .ellipsize(pango::EllipsizeMode::End)
                    .single_line_mode(true)
                    .build();
                self_.sender_bin.set_child(Some(&label));
                label
            };
            let sender_name_expression = message.sender_name_expression();
            sender_name_expression.bind(&label, "label", Some(&label));

            // Remove the previous color css class
            if let Some(class) = self_.sender_color_class.borrow().as_ref() {
                label.remove_css_class(class);
            }

            // Color sender label
            if let MessageSender::User(user) = message.sender() {
                let classes = vec![
                    "sender-text-red",
                    "sender-text-orange",
                    "sender-text-violet",
                    "sender-text-green",
                    "sender-text-cyan",
                    "sender-text-blue",
                    "sender-text-pink",
                ];

                let color_class = classes[user.id() as usize % classes.len()];
                label.add_css_class(color_class);

                self_.sender_color_class.replace(Some(color_class.into()));
            } else {
                self_.sender_color_class.replace(None);
            }
        } else {
            self_.sender_bin.set_child(None::<&gtk::Widget>);
            self_.sender_color_class.replace(None);
        }

        self_.message_label.set_message(message);
    }
}
