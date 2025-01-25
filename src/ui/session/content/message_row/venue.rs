use std::cell::RefCell;
use std::sync::OnceLock;

use glib::clone;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use ui::MessageBaseExt;

use crate::model;
use crate::ui;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/message_row/venue.ui")]
    pub(crate) struct MessageVenue {
        pub(super) message: glib::WeakRef<model::Message>,
        pub(super) handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub(super) map_window: glib::WeakRef<ui::MapWindow>,
        #[template_child]
        pub(super) message_bubble: TemplateChild<ui::MessageBubble>,
        #[template_child]
        pub(super) map: TemplateChild<ui::Map>,
        #[template_child]
        pub(super) title_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageVenue {
        const NAME: &'static str = "PaplMessageVenue";
        type Type = super::MessageVenue;
        type ParentType = ui::MessageBase;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);

            klass.set_css_name("messagevenue");

            klass.install_action("message-row.open", None, move |widget, _, _| {
                widget.open();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageVenue {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![glib::ParamSpecObject::builder::<model::Message>("message")
                    .explicit_notify()
                    .build()]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "message" => self.obj().set_message(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "message" => self.message.upgrade().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for MessageVenue {}
    impl ui::MessageBaseImpl for MessageVenue {}

    #[gtk::template_callbacks]
    impl MessageVenue {
        #[template_callback]
        fn on_map_gesture_click_pressed(&self) {
            let obj = &*self.obj();

            let map_window =
                ui::MapWindow::new(self.obj().root().and_downcast_ref::<gtk::Window>());
            map_window.add_css_class("venue");

            self.map_window.set(Some(&map_window));

            obj.update_map_window(&self.message.upgrade().unwrap());
            map_window.center_marker();

            map_window.present();
        }
    }
}

glib::wrapper! {
    pub(crate) struct MessageVenue(ObjectSubclass<imp::MessageVenue>)
        @extends gtk::Widget, ui::MessageBase;
}

impl MessageBaseExt for MessageVenue {
    type Message = model::Message;

    fn set_message(&self, message: &Self::Message) {
        let imp = self.imp();

        let old_message = imp.message.upgrade();
        if old_message.as_ref() == Some(message) {
            return;
        }

        if let Some(old_message) = old_message {
            let handler_id = imp.handler_id.take().unwrap();
            old_message.disconnect(handler_id);
        }

        imp.message_bubble.update_from_message(message, true);
        imp.message_bubble.add_message_label_class("caption");
        imp.message_bubble.add_message_label_class("dim-label");

        // Update the message.
        let handler_id = message.connect_content_notify(clone!(
            #[weak(rename_to = obj)]
            self,
            move |message| {
                obj.update_row(message);
                obj.update_map_window(message);
            }
        ));
        imp.handler_id.replace(Some(handler_id));
        self.update_row(message);

        imp.message.set(Some(message));
        self.notify("message");
    }
}

impl MessageVenue {
    pub(crate) fn open(&self) {
        let (lat, lon) = self.imp().map.marker_location();

        gtk::UriLauncher::new(&format!(
            "https://www.openstreetmap.org/?mlat={lat}&mlon={lon}"
        ))
        .launch(gtk::Window::NONE, gio::Cancellable::NONE, |_| {});
    }

    fn update_row(&self, message: &model::Message) {
        match message.content().0 {
            tdlib::enums::MessageContent::MessageVenue(td_message) => {
                let imp = self.imp();

                let venue = td_message.venue;

                if let Some(icon_name) = icon_name(&venue) {
                    imp.map
                        .set_custom_marker(Some(ui::IconMapMarker::from(Some(icon_name)).upcast()));
                }

                let location = venue.location;

                imp.map
                    .set_marker_position(location.latitude, location.longitude);
                imp.map.center_marker(16.0);

                imp.title_label.set_text(&venue.title);
                imp.message_bubble.set_label(venue.address);
            }
            _ => unreachable!(),
        }
    }

    fn update_map_window(&self, message: &model::Message) {
        if let Some(map_window) = self.imp().map_window.upgrade() {
            match message.content().0 {
                tdlib::enums::MessageContent::MessageVenue(td_message) => {
                    let location = td_message.venue.location;

                    map_window
                        .map()
                        .set_marker_position(location.latitude, location.longitude);
                }
                _ => unreachable!(),
            }
        }
    }
}

fn icon_name(venue: &tdlib::types::Venue) -> Option<&str> {
    let venue_type = venue.r#type.as_str();

    Some(match venue_type {
        "food/default" => "emoji-food-symbolic",
        _ => return None,
    })
}
