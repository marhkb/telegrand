mod media;
mod media_picture;
mod photo;
mod sticker;
mod sticker_paintable;
mod text;

use self::media::Media;
use self::media_picture::MediaPicture;
pub use self::photo::MessagePhoto;
pub use self::sticker::MessageSticker;
use self::sticker_paintable::StickerPaintable;
pub use self::text::MessageText;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};

use crate::session::chat::{Message, MessageSender, SponsoredMessage};
use crate::session::components::Avatar;
use crate::session::ChatType;

const AVATAR_SIZE: i32 = 32;
const SPACING: i32 = 6;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default)]
    pub struct MessageRow {
        /// A `Message` or `SponsoredMessage`
        pub message: RefCell<Option<glib::Object>>,
        pub content: RefCell<Option<gtk::Widget>>,
        pub avatar: RefCell<Option<Avatar>>,
        pub is_outgoing: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageRow {
        const NAME: &'static str = "ContentMessageRow";
        type Type = super::MessageRow;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for MessageRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "message",
                        "Message",
                        "The message represented by this row",
                        glib::Object::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecObject::new(
                        "content",
                        "Content",
                        "The content widget",
                        gtk::Widget::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "message" => obj.set_message(value.get().unwrap()),
                "content" => obj.set_content(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "message" => obj.message().to_value(),
                "content" => obj.content().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _obj: &Self::Type) {
            if let Some(avatar) = self.avatar.borrow().as_ref() {
                avatar.unparent();
            }

            if let Some(content) = self.content.borrow().as_ref() {
                content.unparent();
            }
        }
    }

    impl WidgetImpl for MessageRow {
        fn measure(
            &self,
            _widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let (mut minimum, mut natural, mut minimum_baseline, mut natural_baseline) =
                (0, 0, -1, -1);
            let mut remaining_for_size = for_size;

            if let Some(avatar) = self.avatar.borrow().as_ref() {
                let (avatar_minimum, avatar_natural, _, _) = avatar.measure(orientation, for_size);
                minimum = avatar_minimum;
                natural = avatar_natural;

                if let gtk::Orientation::Horizontal = orientation {
                    minimum += SPACING;
                    natural += SPACING;
                } else if remaining_for_size != -1 {
                    let (_, avatar_natural_opposite, _, _) =
                        avatar.measure(gtk::Orientation::Horizontal, avatar_natural);
                    remaining_for_size -= avatar_natural_opposite + SPACING;
                }
            }

            if let Some(content) = self.content.borrow().as_ref() {
                let (
                    content_minimum,
                    content_natural,
                    content_minimum_baseline,
                    content_natural_baseline,
                ) = {
                    let (minimum, mut natural, minimum_baseline, natural_baseline) =
                        content.measure(orientation, remaining_for_size);
                    let (_, default_natural, _, _) = content.measure(orientation, -1);

                    // Always prefer default natural size
                    if natural > default_natural && default_natural >= minimum {
                        natural = default_natural
                    }

                    (minimum, natural, minimum_baseline, natural_baseline)
                };

                minimum_baseline = content_minimum_baseline;
                natural_baseline = content_natural_baseline;

                if let gtk::Orientation::Horizontal = orientation {
                    minimum += content_minimum;
                    natural += content_natural;
                } else {
                    minimum = minimum.max(content_minimum);
                    natural = natural.max(content_natural);
                }
            }

            (minimum, natural, minimum_baseline, natural_baseline)
        }

        fn size_allocate(&self, _widget: &Self::Type, width: i32, height: i32, baseline: i32) {
            let mut remaining_width = width;

            if let Some(avatar) = self.avatar.borrow().as_ref() {
                let (_, natural_size) = avatar.preferred_size();
                let allocation = gdk::Rectangle::new(
                    0,
                    height - natural_size.height(),
                    natural_size.width(),
                    natural_size.height(),
                );
                avatar.size_allocate(&allocation, -1);

                remaining_width -= natural_size.width() + SPACING;
            }

            if let Some(content) = self.content.borrow().as_ref() {
                let (_, natural_size) = content.preferred_size();
                let actual_width = remaining_width.min(natural_size.width());
                let x = if self.is_outgoing.get() {
                    width - actual_width
                } else {
                    width - remaining_width
                };

                let allocation = gdk::Rectangle::new(x, 0, actual_width, height);
                content.size_allocate(&allocation, baseline);
            }
        }

        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
    }
}

glib::wrapper! {
    pub struct MessageRow(ObjectSubclass<imp::MessageRow>)
        @extends gtk::Widget;
}

pub trait MessageRowExt: IsA<MessageRow> {
    fn new(message: &glib::Object) -> Self;

    fn message(&self) -> Option<glib::Object> {
        self.upcast_ref().imp().message.borrow().to_owned()
    }

    fn set_message(&self, message: Option<glib::Object>) {
        if self.message() == message {
            return;
        }

        let imp = self.upcast_ref().imp();
        if let Some(ref message) = message {
            if let Some(message) = message.downcast_ref::<Message>() {
                imp.is_outgoing.set(message.is_outgoing());

                let show_avatar = if !message.is_outgoing() {
                    match message.chat().type_() {
                        ChatType::BasicGroup(_) => true,
                        ChatType::Supergroup(supergroup) => !supergroup.is_channel(),
                        _ => false,
                    }
                } else {
                    false
                };
                if show_avatar {
                    let avatar_item = match message.sender() {
                        MessageSender::User(user) => user.avatar().clone(),
                        MessageSender::Chat(chat) => chat.avatar().clone(),
                    };

                    if imp.avatar.borrow().is_none() {
                        let avatar = Avatar::new();
                        avatar.set_size(AVATAR_SIZE);
                        avatar.set_item(Some(avatar_item));
                        avatar.set_parent(self.upcast_ref());
                        imp.avatar.replace(Some(avatar));
                    } else if let Some(avatar) = imp.avatar.borrow().as_ref() {
                        avatar.set_item(Some(avatar_item));
                    }
                } else {
                    if let Some(avatar) = imp.avatar.borrow().as_ref() {
                        avatar.unparent();
                    }
                    imp.avatar.replace(None);
                }
            } else if message.downcast_ref::<SponsoredMessage>().is_some() {
                imp.is_outgoing.set(false);
            } else {
                unreachable!("Unexpected message type: {:?}", message);
            }
        }

        if let Some(content) = imp.content.borrow().as_ref() {
            if imp.is_outgoing.get() {
                content.set_margin_start(AVATAR_SIZE + SPACING);
                content.set_margin_end(0);
                content.add_css_class("outgoing");
            } else {
                content.set_margin_start(0);
                content.set_margin_end(AVATAR_SIZE + SPACING);
                content.remove_css_class("outgoing");
            }
        }

        imp.message.replace(message);
        self.notify("message");
    }

    fn connect_message_notify<F: Fn(&Self, &glib::ParamSpec) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_notify_local(Some("message"), f)
    }

    fn content(&self) -> Option<gtk::Widget> {
        self.upcast_ref().imp().content.borrow().to_owned()
    }

    fn set_content(&self, content: Option<gtk::Widget>) {
        if self.content() == content {
            return;
        }

        let imp = self.upcast_ref().imp();

        if let Some(content) = imp.content.borrow().as_ref() {
            content.unparent();
        }

        if let Some(ref content) = content {
            content.set_parent(self.upcast_ref());
        }

        imp.content.replace(content);
        self.notify("content");
    }
}

impl<T: glib::object::IsClass + IsA<glib::Object> + IsA<MessageRow>> MessageRowExt for T {
    fn new(message: &glib::Object) -> Self {
        glib::Object::new(&[("message", message)]).expect("Failed to create MessageRow")
    }
}

unsafe impl<T: WidgetImpl + ObjectImpl + 'static> IsSubclassable<T> for MessageRow {
    fn class_init(class: &mut glib::Class<Self>) {
        <gtk::Widget as IsSubclassable<T>>::class_init(class.upcast_ref_mut());
    }

    fn instance_init(instance: &mut glib::subclass::InitializingObject<T>) {
        <gtk::Widget as IsSubclassable<T>>::instance_init(instance);
    }
}
