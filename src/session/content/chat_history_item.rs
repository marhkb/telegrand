use gtk::glib;
use gtk::glib::DateTime;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::tdlib::Message;

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "ContentChatHistoryItemType")]
pub(crate) enum ChatHistoryItemType {
    Message(Message),
    DayDivider(DateTime),
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, glib::Enum)]
#[enum_type(name = "MessageStyle")]
pub(crate) enum MessageStyle {
    #[default]
    Single,
    First,
    Last,
    Center,
}

mod imp {
    use super::*;
    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::Cell;

    #[derive(Debug, Default)]
    pub(crate) struct ChatHistoryItem {
        pub(super) type_: OnceCell<ChatHistoryItemType>,
        pub(super) style: Cell<MessageStyle>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatHistoryItem {
        const NAME: &'static str = "ContentChatHistoryItem";
        type Type = super::ChatHistoryItem;
    }

    impl ObjectImpl for ChatHistoryItem {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::builder::<ChatHistoryItemType>("type")
                        .write_only()
                        .construct_only()
                        .build(),
                    glib::ParamSpecEnum::builder::<MessageStyle>("style").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "style" => self.style.get().into(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "type" => {
                    let type_ = value.get::<ChatHistoryItemType>().unwrap();
                    self.type_.set(type_).unwrap();
                }
                "style" => {
                    self.style.set(value.get().unwrap());
                }
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct ChatHistoryItem(ObjectSubclass<imp::ChatHistoryItem>);
}

impl ChatHistoryItem {
    pub(crate) fn for_message(message: Message) -> Self {
        let type_ = ChatHistoryItemType::Message(message);
        glib::Object::builder().property("type", type_).build()
    }

    pub(crate) fn for_day_divider(day: DateTime) -> Self {
        let type_ = ChatHistoryItemType::DayDivider(day);
        glib::Object::builder().property("type", type_).build()
    }

    pub(crate) fn type_(&self) -> &ChatHistoryItemType {
        self.imp().type_.get().unwrap()
    }

    pub(crate) fn style(&self) -> MessageStyle {
        self.imp().style.get()
    }

    pub(crate) fn set_style(&self, style: MessageStyle) {
        self.imp().style.set(style);
    }

    pub(crate) fn is_groupable(&self) -> bool {
        if let Some(message) = self.message() {
            use tdlib::enums::MessageContent::*;
            matches!(
                message.content().0,
                MessageText(_)
                    | MessageAnimation(_)
                    | MessageAudio(_)
                    | MessageDocument(_)
                    | MessagePhoto(_)
                    | MessageSticker(_)
                    | MessageVideo(_)
                    | MessageVideoNote(_)
                    | MessageVoiceNote(_)
                    | MessageLocation(_)
                    | MessageVenue(_)
                    | MessageContact(_)
                    | MessageAnimatedEmoji(_)
                    | MessageDice(_)
                    | MessageGame(_)
                    | MessagePoll(_)
                    | MessageInvoice(_)
                    | MessageCall(_)
                    | MessageUnsupported
            )
        } else {
            false
        }
    }

    pub(crate) fn group_key(&self) -> Option<(bool, i64)> {
        if self.is_groupable() {
            self.message().map(|m| (m.is_outgoing(), m.sender().id()))
        } else {
            None
        }
    }

    pub(crate) fn message(&self) -> Option<&Message> {
        if let ChatHistoryItemType::Message(message) = self.type_() {
            Some(message)
        } else {
            None
        }
    }

    pub(crate) fn message_timestamp(&self) -> Option<DateTime> {
        if let ChatHistoryItemType::Message(message) = self.type_() {
            Some(
                glib::DateTime::from_unix_utc(message.date().into())
                    .and_then(|t| t.to_local())
                    .unwrap(),
            )
        } else {
            None
        }
    }
}
