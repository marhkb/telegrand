use crate::tdlib::ChatMember;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};

use super::Avatar;
use crate::{expressions, strings};
use tdlib::enums::{UserStatus, UserType};

mod imp {
    use super::*;
    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/components-chat-member-row.ui")]
    pub(crate) struct ChatMemberRow {
        #[template_child]
        pub(super) avatar: TemplateChild<Avatar>,
        #[template_child]
        pub(super) user_name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) member_status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) user_status_label: TemplateChild<gtk::Inscription>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatMemberRow {
        const NAME: &'static str = "ComponentsChatMemberRow";
        type Type = super::ChatMemberRow;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("chatmember");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChatMemberRow {
        fn dispose(&self) {
            self.avatar.unparent();
            self.user_status_label.parent().unwrap().unparent();
        }
    }
    
    impl WidgetImpl for ChatMemberRow {}

    impl BoxImpl for ChatMemberRow {}
}

glib::wrapper! {
    pub(crate) struct ChatMemberRow(ObjectSubclass<imp::ChatMemberRow>)
        @extends gtk::Widget;
}

impl ChatMemberRow {
    pub fn new() -> Self {
        glib::Object::new(&[])
    }
    pub fn bind_member(&self, member: ChatMember) {
        let imp = self.imp();

        let user = member.user();

        let user_expression = gtk::ObjectExpression::new(&user);
        let name_expression = expressions::user_display_name(&user_expression);
        name_expression.bind(&*imp.user_name_label, "label", Some(&user));

        if let UserType::Bot(_) = user.type_().0 {
            imp.user_status_label.set_text(Some(&gettext("bot")));
        } else {
            let status = user.status().0;
            let status_label = &*imp.user_status_label;

            match status {
                UserStatus::Online(_) => status_label.set_css_classes(&["accent"]),
                _ => status_label.set_css_classes(&["dim-label"]),
            }

            let status = strings::user_status(&status);
            imp.user_status_label.set_text(Some(&status));
        };

        imp.member_status_label.set_label(&member.status());

        imp.avatar.set_item(Some(user.upcast()));
    }
}
