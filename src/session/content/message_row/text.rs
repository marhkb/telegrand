use gettextrs::gettext;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use tdgrand::enums::MessageContent;

use crate::session::{
    chat::{BoxedMessageContent, Message, MessageSender, SponsoredMessage},
    content::{MessageRow, MessageRowExt},
    Chat, ChatType,
};
use crate::utils::parse_formatted_text;

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-text.ui")]
    pub struct MessageText {
        pub sender_color_class: RefCell<Option<String>>,
        pub bindings: RefCell<Vec<gtk::ExpressionWatch>>,
        #[template_child]
        pub sender_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub content_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageText {
        const NAME: &'static str = "ContentMessageText";
        type Type = super::MessageText;
        type ParentType = MessageRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageText {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.connect_message_notify(|obj, _| obj.update_widget());
        }
    }

    impl WidgetImpl for MessageText {}
}

glib::wrapper! {
    pub struct MessageText(ObjectSubclass<imp::MessageText>)
        @extends gtk::Widget, MessageRow;
}

impl MessageText {
    fn update_widget(&self) {
        if let Some(message) = self.message() {
            let self_ = imp::MessageText::from_instance(self);
            let mut bindings = self_.bindings.borrow_mut();

            while let Some(binding) = bindings.pop() {
                binding.unwatch();
            }

            // Remove the previous color css class
            let mut sender_color_class = self_.sender_color_class.borrow_mut();
            if let Some(class) = sender_color_class.as_ref() {
                self_.sender_label.remove_css_class(class);
                *sender_color_class = None;
            }

            let content_expression = gtk::PropertyExpression::new(
                Message::static_type(),
                gtk::NONE_EXPRESSION,
                "content",
            );
            let text_expression = gtk::ClosureExpression::new(
                |args| -> String {
                    let content = args[1].get::<BoxedMessageContent>().unwrap();
                    format_message_content_text(content.0)
                },
                &[content_expression.upcast()],
            );

            if let Some(message) = message.downcast_ref::<Message>() {
                // Show sender label, if needed
                let show_sender = {
                    if message.is_outgoing() {
                        matches!(message.sender(), MessageSender::Chat(_))
                    } else {
                        matches!(
                            message.chat().type_(),
                            ChatType::BasicGroup(_) | ChatType::Supergroup(_)
                        )
                    }
                };
                if show_sender {
                    let sender_name_expression = message.sender_name_expression();
                    let sender_binding = sender_name_expression.bind(
                        &*self_.sender_label,
                        "label",
                        gtk::NONE_WIDGET,
                    );
                    bindings.push(sender_binding);

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
                        self_.sender_label.add_css_class(color_class);

                        *sender_color_class = Some(color_class.into());
                    }

                    self_.sender_label.set_visible(true);
                } else {
                    self_.sender_label.set_visible(false);
                }

                // Set content label expression
                let text_binding =
                    text_expression.bind(&*self_.content_label, "label", Some(message));
                bindings.push(text_binding);
            } else if let Some(sponsored_message) = message.downcast_ref::<SponsoredMessage>() {
                self_.sender_label.set_visible(true);

                let chat_title_expression = gtk::PropertyExpression::new(
                    Chat::static_type(),
                    gtk::NONE_EXPRESSION,
                    "title",
                );
                let sender_binding = chat_title_expression.bind(
                    &*self_.sender_label,
                    "label",
                    Some(&sponsored_message.sponsor_chat()),
                );
                bindings.push(sender_binding);

                let text_binding =
                    text_expression.bind(&*self_.content_label, "label", Some(sponsored_message));
                bindings.push(text_binding);
            } else {
                unreachable!("Unexpected message type: {:?}", message);
            }
        }
    }
}

fn format_message_content_text(content: MessageContent) -> String {
    match content {
        MessageContent::MessageText(content) => parse_formatted_text(content.text),
        _ => format!("<i>{}</i>", gettext("This message is unsupported")),
    }
}
