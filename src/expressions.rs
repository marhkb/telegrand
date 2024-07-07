use gettextrs::gettext;
use glib::prelude::*;
use gtk::glib;
use gtk::glib::closure;
use gtk::prelude::GObjectPropertyExpressionExt;

use crate::model;

/// Creates an expression that produces the display name of a chat. This will either produce the
/// title of the chat or the translated "Saved Messages" string in the case of the own chat.
pub(crate) fn chat_display_name(chat_expression: &gtk::Expression) -> gtk::Expression {
    let title_expression = chat_expression.chain_property::<model::Chat>("title");
    let is_deleted_expression = is_deleted_expression(chat_expression);
    gtk::ClosureExpression::with_callback(
        [chat_expression, &title_expression, &is_deleted_expression],
        |args| {
            let chat = args[1].get::<model::Chat>().unwrap();
            let title = args[2].get::<String>().unwrap();
            let is_deleted = args[3].get::<bool>().unwrap();
            if chat.is_own_chat() {
                gettext("Saved Messages")
            } else if chat.is_replies_chat() {
                gettext("Replies")
            } else if is_deleted {
                gettext("Deleted Account")
            } else {
                title
            }
        },
    )
    .upcast()
}

/// Creates an expression that produces the full name of an user, binding both the
/// first-name and last-name property together.
pub(crate) fn user_display_name(user_expression: &gtk::Expression) -> gtk::Expression {
    let first_name_expression = user_expression.chain_property::<model::User>("first-name");
    let last_name_expression = user_expression.chain_property::<model::User>("last-name");
    let type_expression = user_expression.chain_property::<model::User>("user-type");
    gtk::ClosureExpression::with_callback(
        &[first_name_expression, last_name_expression, type_expression],
        |args| {
            let first_name = args[1].get::<String>().unwrap();
            let last_name = args[2].get::<String>().unwrap();
            let user_type = args[3].get::<model::BoxedUserType>().unwrap().0;
            if let tdlib::enums::UserType::Deleted = user_type {
                gettext("Deleted Account")
            } else {
                first_name + " " + &last_name
            }
        },
    )
    .upcast()
}

pub(crate) fn is_deleted_expression(chat_expression: &gtk::Expression) -> gtk::Expression {
    gtk::ClosureExpression::with_callback([chat_expression], |args| {
        let chat = args[1].get::<model::Chat>().unwrap();
        matches!(chat.chat_type(), model::ChatType::Private(user) if user.user_type().0 == tdlib::enums::UserType::Deleted)
    })
    .upcast()
}

pub(crate) fn restriction_expression(chat: &model::Chat) -> gtk::Expression {
    match chat.chat_type() {
        model::ChatType::Supergroup(data) if !data.is_channel() => {
            restriction_label_expression::<model::Supergroup, _>(&data)
        }
        model::ChatType::BasicGroup(data) => {
            restriction_label_expression::<model::BasicGroup, _>(&data)
        }
        _ => gtk::ConstantExpression::new("").upcast(),
    }
}

fn restriction_label_expression<T: StaticType, V: ToValue>(value: &V) -> gtk::Expression {
    let member_status_expression = gtk::PropertyExpression::new(
        T::static_type(),
        Some(gtk::ConstantExpression::new(value)),
        "status",
    );
    let permissions_expression = model::Chat::this_expression("permissions");

    gtk::ClosureExpression::new::<String>(
        &[member_status_expression, permissions_expression],
        closure!(|_: glib::Object, status: model::BoxedChatMemberStatus, chat_permissions: model::BoxedChatPermissions| {
            if chat_permissions.0.can_send_basic_messages {
                match status.0 {
                    tdlib::enums::ChatMemberStatus::Restricted(status) if !status.permissions.can_send_basic_messages => {
                        if status.restricted_until_date == 0 {
                            gettext("The admins of this group have restricted you from writing here")
                        } else {
                            let date =
                            glib::DateTime::from_unix_utc(status.restricted_until_date.into()).unwrap();

                            gettext!(
                                "The admins of this group have restricted you from writing here until {}",
                                date.format(&if glib::DateTime::now_local()
                                    .unwrap()
                                    .difference(&date)
                                    .as_days()
                                    == 0
                                {
                                    gettext("%l:%M %p")
                                } else {
                                    // Translators: This is a date and time representation, without seconds.
                                    // Here you may want to change to a 24-hours representation and change order, based on your locale.
                                    // You can use this site to learn more: https://www.strfti.me/
                                    gettext("%B %e, %Y %l:%M %p")
                                })
                                .unwrap()
                                .to_string()
                            )
                        }
                    }
                    _ => String::new(),
                }
            } else if !matches!(status.0, tdlib::enums::ChatMemberStatus::Creator(_)
                | tdlib::enums::ChatMemberStatus::Administrator(_))
            {
                gettext("Writing messages isn't allowed in this group.")
            } else {
                String::new()
            }
        }),
    )
    .upcast()
}
