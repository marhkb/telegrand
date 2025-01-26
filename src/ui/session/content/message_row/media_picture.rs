use std::cell::Cell;
use std::sync::OnceLock;

use glib::clone;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

const MAX_HEIGHT: i32 = 350;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/message_row/media_picture.ui")]
    pub(crate) struct MediaPicture {
        pub(super) aspect_ratio: Cell<f64>,
        #[template_child]
        pub(super) picture: TemplateChild<gtk::Picture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaPicture {
        const NAME: &'static str = "PaplMessageMediaPicture";
        type Type = super::MediaPicture;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("mediapicture");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MediaPicture {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecObject::builder::<gdk::Paintable>("paintable")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecDouble::builder("aspect-ratio")
                        .explicit_notify()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "paintable" => obj.set_paintable(value.get::<Option<&gdk::Paintable>>().unwrap()),
                "aspect-ratio" => obj.set_aspect_ratio(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "paintable" => obj.paintable().to_value(),
                "aspect-ratio" => obj.aspect_ratio().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_overflow(gtk::Overflow::Hidden);

            self.picture.connect_paintable_notify(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.notify("paintable");
                }
            ));
        }

        fn dispose(&self) {
            self.picture.unparent();
        }
    }

    impl WidgetImpl for MediaPicture {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let minimum = self.picture.measure(orientation, for_size).0;

            if let gtk::Orientation::Horizontal = orientation {
                let natural = if for_size < 0 {
                    (MAX_HEIGHT as f64 * self.aspect_ratio.get()) as i32
                } else {
                    let adjusted_for_size = for_size.min(MAX_HEIGHT);
                    (adjusted_for_size as f64 * self.aspect_ratio.get()) as i32
                }
                .max(minimum);

                (minimum, natural, -1, -1)
            } else {
                let natural = if for_size < 0 {
                    MAX_HEIGHT
                } else {
                    let natural = (for_size as f64 / self.aspect_ratio.get()) as i32;
                    natural.clamp(minimum, MAX_HEIGHT)
                };

                (minimum, natural, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.picture.allocate(width, height, baseline, None);
        }

        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
    }
}

glib::wrapper! {
    pub(crate) struct MediaPicture(ObjectSubclass<imp::MediaPicture>)
        @extends gtk::Widget;
}

impl MediaPicture {
    pub(crate) fn paintable(&self) -> Option<gdk::Paintable> {
        self.imp().picture.paintable()
    }

    pub(crate) fn set_paintable(&self, paintable: Option<&impl IsA<gdk::Paintable>>) {
        self.imp().picture.set_paintable(paintable);
    }

    pub(crate) fn aspect_ratio(&self) -> f64 {
        self.imp().aspect_ratio.get()
    }

    pub(crate) fn set_aspect_ratio(&self, aspect_ratio: f64) {
        if self.aspect_ratio() == aspect_ratio {
            return;
        }

        self.imp().aspect_ratio.set(aspect_ratio);
        self.queue_resize();

        self.notify("aspect-ratio");
    }
}
