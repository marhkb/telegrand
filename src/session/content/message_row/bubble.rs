use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use adw::prelude::*;
use glib::clone;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use once_cell::sync::Lazy;

use crate::components::Background;
use crate::session::content::message_row::MessageIndicators;
use crate::session::content::message_row::MessageLabel;
use crate::session::content::message_row::MessageReply;
use crate::tdlib::Chat;
use crate::tdlib::ChatType;
use crate::tdlib::Message;
use crate::tdlib::MessageSender;
use crate::tdlib::SponsoredMessage;

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
    #[template(string = r#"
    using Adw 1;

    template $MessageBubble {
        overflow: hidden;

        Overlay overlay {
            Box {
                orientation: vertical;

                Label sender_label {
                    styles ["caption-heading"]

                    ellipsize: end;
                    xalign: 0;
                    visible: false;
                }

                Adw.Bin message_reply_bin {}

                Adw.Bin prefix_bin {}

                $MessageLabel message_label {
                    visible: false;
                }
            }

            [overlay]
            $MessageIndicators indicators {
                halign: end;
                valign: end;
            }
        }
    }
    "#)]
    pub(crate) struct MessageBubble {
        pub(super) sender_color_class: RefCell<Option<String>>,
        pub(super) sender_binding: RefCell<Option<gtk::ExpressionWatch>>,
        pub(super) parent_list_view: RefCell<glib::WeakRef<gtk::ListView>>,
        pub(super) parent_background: RefCell<glib::WeakRef<Background>>,
        #[template_child]
        pub(super) overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub(super) sender_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) message_reply_bin: TemplateChild<adw::Bin>,
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
        fn realize(&self) {
            self.parent_realize();

            let widget = self.obj();

            if let Some(view) = widget.parent_list_view() {
                self.parent_list_view.replace(view.downgrade());
                view.vadjustment()
                    .unwrap()
                    .connect_value_notify(clone!(@weak widget => move |_| {
                        widget.queue_draw();
                    }));
            }

            if let Some(background) = widget.parent_background() {
                self.parent_background.replace(background.downgrade());
                background.subscribe_to_redraw(widget.upcast_ref());
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();

            if let Some(background) = self.parent_background.borrow().upgrade() {
                if !background.has_css_class("fallback") {
                    let bounds = {
                        let width = widget.width() as f32;
                        let height = widget.height() as f32;

                        gtk::graphene::Rect::new(0.0, 0.0, width, height)
                    };

                    let gradient_bounds = background.compute_bounds(self.obj().as_ref()).unwrap();

                    if widget.has_css_class("outgoing") {
                        snapshot
                            .append_node(&background.message_bg_node(&bounds, &gradient_bounds));
                    } else {
                        snapshot.push_opacity(0.1);
                        snapshot.append_node(&background.bg_node(&bounds, &gradient_bounds));
                        snapshot.pop();
                    };
                }
            }

            self.parent_snapshot(snapshot);
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

        // Handle MessageReply
        if message.reply_to_message_id() != 0 {
            let reply = MessageReply::new(message);

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

    fn parent_list_view(&self) -> Option<gtk::ListView> {
        self.ancestor(gtk::ListView::static_type())?.downcast().ok()
    }

    fn parent_background(&self) -> Option<Background> {
        self.ancestor(Background::static_type())?.downcast().ok()
    }
}
