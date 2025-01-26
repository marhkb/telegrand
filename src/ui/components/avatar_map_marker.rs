use std::sync::OnceLock;

use glib::clone;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::ui;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/components/avatar_map_marker.ui")]
    pub(crate) struct AvatarMapMarker {
        #[template_child]
        pub(super) avatar: TemplateChild<ui::Avatar>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AvatarMapMarker {
        const NAME: &'static str = "PaplAvatarMapMarker";
        type Type = super::AvatarMapMarker;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("avatarmapmarker");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AvatarMapMarker {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![glib::ParamSpecObject::builder::<model::User>("user")
                    .construct()
                    .explicit_notify()
                    .build()]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "user" => self.obj().set_user(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "user" => self.obj().user().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            self.avatar.connect_notify_local(
                Some("item"),
                clone!(
                    #[weak]
                    obj,
                    move |_, _| {
                        obj.notify("user");
                    }
                ),
            );
        }

        fn dispose(&self) {
            let mut child = self.obj().first_child();
            while let Some(child_) = child {
                child = child_.next_sibling();
                child_.unparent();
            }
        }
    }

    impl WidgetImpl for AvatarMapMarker {}
}

glib::wrapper! {
    pub(crate) struct AvatarMapMarker(ObjectSubclass<imp::AvatarMapMarker>)
        @extends gtk::Widget;
}

impl From<&model::User> for AvatarMapMarker {
    fn from(user: &model::User) -> Self {
        glib::Object::builder().property("user", user).build()
    }
}

impl AvatarMapMarker {
    pub(crate) fn user(&self) -> Option<model::User> {
        self.imp()
            .avatar
            .item()
            .map(|item| item.downcast().unwrap())
    }

    pub(crate) fn set_user(&self, user: model::User) {
        if self.user().as_ref() == Some(&user) {
            return;
        }
        self.imp().avatar.set_item(Some(user.upcast()));
    }
}
