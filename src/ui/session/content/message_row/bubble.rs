use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::OnceLock;

use adw::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::ui;
use crate::utils;

const MAX_WIDTH: i32 = 400;
const SENDER_COLOR_CLASSES: &[&str] = &[
    "sender-text-red",
    "sender-text-orange",
    "sender-text-violet",
    "sender-text-green",
    "sender-text-cyan",
    "sender-text-blue",
    "sender-text-pink",
];

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/message_row/bubble.ui")]
    pub(crate) struct MessageBubble {
        pub(super) sender_color_class: RefCell<Option<String>>,
        pub(super) sender_binding: RefCell<Option<gtk::ExpressionWatch>>,
        #[template_child]
        pub(super) box_: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub(super) sender_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) message_reply_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) prefix_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) message_label: TemplateChild<ui::MessageLabel>,
        #[template_child]
        pub(super) indicators: TemplateChild<ui::MessageIndicators>,
        #[template_child]
        pub(super) suffix_bin: TemplateChild<adw::Bin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageBubble {
        const NAME: &'static str = "PaplMessageBubble";
        type Type = super::MessageBubble;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("messagebubble");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageBubble {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecObject::builder::<gtk::Widget>("prefix")
                        .write_only()
                        .build(),
                    glib::ParamSpecString::builder("label").write_only().build(),
                    glib::ParamSpecObject::builder::<gtk::Widget>("suffix")
                        .write_only()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "prefix" => obj.set_prefix(value.get().unwrap()),
                "label" => obj.set_label(value.get().unwrap()),
                "suffix" => obj.set_suffix(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self) {
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for MessageBubble {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            // Limit the widget width
            if orientation == gtk::Orientation::Horizontal {
                let (minimum, natural, minimum_baseline, natural_baseline) =
                    self.box_.measure(orientation, for_size);

                (
                    minimum.min(MAX_WIDTH),
                    natural.min(MAX_WIDTH),
                    minimum_baseline,
                    natural_baseline,
                )
            } else {
                let adjusted_for_size = for_size.min(MAX_WIDTH);
                self.box_.measure(orientation, adjusted_for_size)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.box_.allocate(width, height, baseline, None);
        }

        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
    }
}

glib::wrapper! {
    pub(crate) struct MessageBubble(ObjectSubclass<imp::MessageBubble>)
        @extends gtk::Widget;
}

impl MessageBubble {
    pub(crate) fn update_from_message(&self, message: &model::Message, force_hide_sender: bool) {
        let imp = self.imp();

        imp.indicators.set_message(message.upcast_ref());

        let is_channel = if let model::ChatType::Supergroup(data) = message.chat_().chat_type() {
            data.is_channel()
        } else {
            false
        };

        if message.is_outgoing() && !is_channel {
            self.add_css_class("outgoing");
        } else {
            self.remove_css_class("outgoing");
        }

        if let Some(binding) = imp.sender_binding.take() {
            binding.unwatch();
        }

        let show_sender = if force_hide_sender {
            None
        } else if message.chat_().is_own_chat() || message.chat_().is_replies_chat() {
            if message.is_outgoing() {
                None
            } else {
                Some(message.forward_info().unwrap().origin().id())
            }
        } else if message.is_outgoing() {
            if matches!(message.sender(), model::MessageSender::Chat(_)) {
                Some(Some(message.sender().id()))
            } else {
                None
            }
        } else if matches!(
            message.chat_().chat_type(),
            model::ChatType::BasicGroup(_) | model::ChatType::Supergroup(_)
        ) {
            Some(Some(message.sender().id()))
        } else {
            None
        };

        // Handle MessageReply
        if matches!(
            message.reply_to(),
            Some(model::BoxedMessageReplyTo(
                tdlib::enums::MessageReplyTo::Message(_)
            ))
        ) {
            let reply = ui::MessageReply::new(message);

            // FIXME: Do not show message reply when message is being deleted
            imp.message_reply_bin.set_child(Some(&reply));

            self.add_css_class("with-reply");
        } else {
            imp.message_reply_bin.set_child(gtk::Widget::NONE);

            self.remove_css_class("with-reply");
        }

        // Show sender label, if needed
        if let Some(maybe_id) = show_sender {
            let sender_name_expression = message.sender_display_name_expression();
            let sender_binding =
                sender_name_expression.bind(&*imp.sender_label, "label", glib::Object::NONE);
            imp.sender_binding.replace(Some(sender_binding));

            self.update_sender_color(maybe_id);

            imp.sender_label.set_visible(true);
        } else {
            if let Some(old_class) = imp.sender_color_class.take() {
                imp.sender_label.remove_css_class(&old_class);
            }

            imp.sender_label.set_label("");
            imp.sender_label.set_visible(false);
        }
    }

    pub(crate) fn update_from_sponsored_message(
        &self,
        sponsored_message: &model::SponsoredMessage,
    ) {
        let imp = self.imp();

        imp.indicators.set_message(sponsored_message.upcast_ref());

        self.remove_css_class("outgoing");

        if let Some(binding) = imp.sender_binding.take() {
            binding.unwatch();
        }

        imp.sender_label
            .set_label(&sponsored_message.sponsor_label());

        let label_hash = hash_label(&sponsored_message.sponsor_label());

        self.update_sender_color(Some(label_hash));

        imp.sender_label.set_visible(true);
    }

    pub(crate) fn set_prefix(&self, prefix: Option<&gtk::Widget>) {
        self.imp().prefix_bin.set_child(prefix);
    }

    pub(crate) fn set_label(&self, label: String) {
        let imp = self.imp();

        if label.is_empty() {
            imp.message_label.set_label(String::new());
            imp.message_label.set_visible(false);

            self.remove_css_class("with-label");
        } else {
            imp.message_label.set_label(label);
            imp.message_label.set_visible(true);

            self.add_css_class("with-label");
        }

        self.update_indicators_position();
    }

    pub(crate) fn add_message_label_class(&self, class: &str) {
        self.imp().message_label.add_label_class(class);
    }

    pub(crate) fn set_suffix(&self, prefix: Option<&gtk::Widget>) {
        self.imp().suffix_bin.set_child(prefix);
    }

    fn update_sender_color(&self, sender_id: Option<i64>) {
        let imp = self.imp();

        if let Some(old_class) = imp.sender_color_class.take() {
            imp.sender_label.remove_css_class(&old_class);
        }

        let color_class = SENDER_COLOR_CLASSES[sender_id
            .map(|id| id as usize)
            .unwrap_or_else(|| hash_label(&imp.sender_label.label()) as usize)
            % SENDER_COLOR_CLASSES.len()];

        imp.sender_label.add_css_class(color_class);
        imp.sender_color_class.replace(Some(color_class.into()));
    }

    fn update_indicators_position(&self) {
        let imp = self.imp();

        if imp.message_label.label().is_empty() && imp.message_label.indicators().is_some() {
            imp.message_label.set_indicators(None);
            imp.overlay.add_overlay(&*imp.indicators);
        } else if !imp.message_label.label().is_empty() && imp.message_label.indicators().is_none()
        {
            imp.overlay.remove_overlay(&*imp.indicators);
            imp.message_label
                .set_indicators(Some(imp.indicators.clone()));
        }
    }
}

fn hash_label(label: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    label.hash(&mut hasher);
    hasher.finish() as i64
}
