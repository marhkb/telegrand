use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use once_cell::sync::Lazy;

use crate::model;
use crate::strings;
use crate::ui;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub(crate) struct ChatHistoryRow {
        /// A `Message` or `SponsoredMessage`
        pub(super) item: RefCell<Option<glib::Object>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatHistoryRow {
        const NAME: &'static str = "PaplChatHistoryRow";
        type Type = super::ChatHistoryRow;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for ChatHistoryRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::builder::<glib::Object>("item")
                    .explicit_notify()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "item" => obj.set_item(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "item" => obj.item().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for ChatHistoryRow {}
    impl BinImpl for ChatHistoryRow {}
}

glib::wrapper! {
    pub(crate) struct ChatHistoryRow(ObjectSubclass<imp::ChatHistoryRow>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for ChatHistoryRow {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatHistoryRow {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }

    pub(crate) fn item(&self) -> Option<glib::Object> {
        self.imp().item.borrow().to_owned()
    }

    pub(crate) fn set_item(&self, item: Option<glib::Object>) {
        if self.item() == item {
            return;
        }

        if let Some(ref item) = item {
            if let Some(message) = item.downcast_ref::<model::Message>() {
                use tdlib::enums::MessageContent::*;

                match message.content().0 {
                    MessageExpiredPhoto
                    | MessageExpiredVideo
                    | MessageCall(_)
                    | MessageBasicGroupChatCreate(_)
                    | MessageSupergroupChatCreate(_)
                    | MessageChatChangeTitle(_)
                    | MessageChatChangePhoto(_) // TODO: Show photo thumbnail
                    | MessageChatDeletePhoto
                    | MessageChatAddMembers(_)
                    | MessageChatJoinByLink
                    | MessageChatJoinByRequest
                    | MessageChatDeleteMember(_)
                    | MessagePinMessage(_)
                    | MessageScreenshotTaken
                    | MessageGameScore(_)
                    | MessageContactRegistered => {
                        self.get_or_create_event_row()
                            .set_label(&strings::message_content(message));
                    }
                    _ => self.update_or_create_message_row(message.to_owned().upcast()),
                }
            } else if let Some(sponsored_message) = item.downcast_ref::<model::SponsoredMessage>() {
                let content = &sponsored_message.content().0;
                if !matches!(content, tdlib::enums::MessageContent::MessageText(_)) {
                    log::warn!("Unexpected sponsored message of type: {:?}", content);
                }

                self.update_or_create_message_row(sponsored_message.to_owned().upcast());
            } else {
                unreachable!("Unexpected item type: {:?}", item);
            }
        }

        self.imp().item.replace(item);
        self.notify("item");
    }

    fn update_or_create_message_row(&self, message: glib::Object) {
        match self
            .child()
            .and_then(|w| w.downcast::<ui::MessageRow>().ok())
        {
            Some(child) => child.set_message(message),
            None => {
                let child = ui::MessageRow::new(&message);
                self.set_child(Some(&child));
            }
        }
    }

    fn get_or_create_event_row(&self) -> ui::EventRow {
        if let Some(Ok(child)) = self.child().map(|w| w.downcast::<ui::EventRow>()) {
            child
        } else {
            let child = ui::EventRow::new();
            self.set_child(Some(&child));
            child
        }
    }
}
