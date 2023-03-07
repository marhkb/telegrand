use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::session::content::message_row::{MessageIndicators, MessageLabel, MessageRow};
use crate::session::content::MessageStyle;
use crate::tdlib::{Chat, ChatType, Message, MessageSender, SponsoredMessage};

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
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(string = r#"
    using Adw 1;

    template MessageBubble {
        Overlay overlay {
            Box {
                orientation: vertical;

                Label sender_label {
                    styles ["caption-heading"]

                    ellipsize: end;
                    xalign: 0;
                    visible: false;
                }

                Adw.Bin prefix_bin {}

                .MessageLabel message_label {
                    visible: false;
                }
            }

            [overlay]
            .MessageIndicators indicators {
                halign: end;
                valign: end;
            }
        }
    }
    "#)]
    pub(crate) struct MessageBubble {
        pub(super) sender_color_class: RefCell<Option<String>>,
        pub(super) sender_binding: RefCell<Option<gtk::ExpressionWatch>>,
        #[template_child]
        pub(super) overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub(super) sender_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) prefix_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) message_label: TemplateChild<MessageLabel>,
        #[template_child]
        pub(super) indicators: TemplateChild<MessageIndicators>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageBubble {
        const NAME: &'static str = "MessageBubble";
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
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<gtk::Widget>("prefix")
                        .write_only()
                        .build(),
                    glib::ParamSpecString::builder("label").write_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "prefix" => obj.set_prefix(value.get().unwrap()),
                "label" => obj.set_label(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self) {
            self.overlay.unparent();
        }
    }

    impl WidgetImpl for MessageBubble {
        fn map(&self) {
            self.parent_map();
            if let Some(style) = self.style() {
                use MessageStyle::*;
                let label_is_visible = match style {
                    Single | First => true,
                    Last | Center => false,
                };
                if self.sender_color_class.borrow().is_some() {
                    self.sender_label.set_visible(label_is_visible);
                }
            }
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            // Limit the widget width
            if orientation == gtk::Orientation::Horizontal {
                let (minimum, natural, minimum_baseline, natural_baseline) =
                    self.overlay.measure(orientation, for_size);

                (
                    minimum.min(MAX_WIDTH),
                    natural.min(MAX_WIDTH),
                    minimum_baseline,
                    natural_baseline,
                )
            } else {
                let adjusted_for_size = for_size.min(MAX_WIDTH);
                self.overlay.measure(orientation, adjusted_for_size)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.overlay.allocate(width, height, baseline, None);
        }

        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
    }

    impl MessageBubble {
        fn style(&self) -> Option<MessageStyle> {
            self.obj()
                .parent()?
                .parent()?
                .downcast::<MessageRow>()
                .ok()?
                .message_style()
        }
    }
}

glib::wrapper! {
    pub(crate) struct MessageBubble(ObjectSubclass<imp::MessageBubble>)
        @extends gtk::Widget;
}

impl MessageBubble {
    pub(crate) fn update_from_message(&self, message: &Message, force_hide_sender: bool) {
        let imp = self.imp();

        imp.indicators.set_message(message.clone().upcast());

        let is_channel = if let ChatType::Supergroup(data) = message.chat().type_() {
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
        } else if message.chat().is_own_chat() {
            if message.is_outgoing() {
                None
            } else {
                Some(message.forward_info().unwrap().origin().id())
            }
        } else if message.is_outgoing() {
            if matches!(message.sender(), MessageSender::Chat(_)) {
                Some(Some(message.sender().id()))
            } else {
                None
            }
        } else if matches!(
            message.chat().type_(),
            ChatType::BasicGroup(_) | ChatType::Supergroup(_)
        ) {
            Some(Some(message.sender().id()))
        } else {
            None
        };

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

    pub(crate) fn update_from_sponsored_message(&self, sponsored_message: &SponsoredMessage) {
        let imp = self.imp();

        imp.indicators
            .set_message(sponsored_message.clone().upcast());

        self.remove_css_class("outgoing");

        if let Some(binding) = imp.sender_binding.take() {
            binding.unwatch();
        }

        let sender_binding = Chat::this_expression("title").bind(
            &*imp.sender_label,
            "label",
            Some(&sponsored_message.sponsor_chat()),
        );
        imp.sender_binding.replace(Some(sender_binding));

        self.update_sender_color(Some(sponsored_message.sponsor_chat().id()));

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

    fn update_sender_color(&self, sender_id: Option<i64>) {
        let imp = self.imp();

        if let Some(old_class) = imp.sender_color_class.take() {
            imp.sender_label.remove_css_class(&old_class);
        }

        let color_class =
            SENDER_COLOR_CLASSES[sender_id.map(|id| id as usize).unwrap_or_else(|| {
                let mut s = DefaultHasher::new();
                imp.sender_label.label().hash(&mut s);
                s.finish() as usize
            }) % SENDER_COLOR_CLASSES.len()];

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
