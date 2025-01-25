use std::cell::Cell;
use std::f64;
use std::sync::OnceLock;

use glib::clone;
use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::gsk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub(crate) struct CircularProgressBar {
        pub(super) percentage: Cell<f64>,
        pub(super) border_thickness: Cell<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CircularProgressBar {
        const NAME: &'static str = "PaplCircularProgressBar";
        type Type = super::CircularProgressBar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("circularprogressbar");
        }
    }

    impl ObjectImpl for CircularProgressBar {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecDouble::builder("percentage")
                        .maximum(1.0)
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecUInt::builder("border-thickness")
                        .explicit_notify()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "percentage" => self.obj().set_percentage(value.get().unwrap()),
                "border-thickness" => self.obj().set_border_thickness(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "percentage" => self.obj().percentage().to_value(),
                "border-thickness" => self.obj().border_thickness().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            obj.set_halign(gtk::Align::Center);
            obj.set_valign(gtk::Align::Center);

            let adw_style_manager = adw::StyleManager::default();
            adw_style_manager.connect_high_contrast_notify(clone!(
                #[weak]
                obj,
                move |_| obj.queue_draw()
            ));
            adw_style_manager.connect_dark_notify(clone!(
                #[weak]
                obj,
                move |_| obj.queue_draw()
            ));
        }
    }

    impl WidgetImpl for CircularProgressBar {
        fn measure(&self, _: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
            let obj = &*self.obj();

            let width = obj.width_request();
            let height = obj.width_request();

            let size = std::cmp::max(16, std::cmp::min(width, height));

            (size, size, -1, -1)
        }

        #[allow(deprecated)]
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = &*self.obj();

            let style_context = obj.style_context();

            let size = obj.width() as f32;
            let rect = graphene::Rect::new(0.0, 0.0, size, size);

            let child_snapshot = gtk::Snapshot::new();

            child_snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, size / 2.0));

            let percentage = obj.percentage() as f32;

            let color = style_context.color();
            let mut color_alpha = color;
            color_alpha.set_alpha(0.3);

            child_snapshot.append_conic_gradient(
                &rect,
                &graphene::Point::new(size / 2.0, size / 2.0),
                percentage,
                &[
                    gsk::ColorStop::new(percentage, color),
                    gsk::ColorStop::new(percentage, color_alpha),
                ],
            );

            child_snapshot.pop();

            snapshot.push_mask(gsk::MaskMode::InvertedAlpha);

            let border_thickness = obj.border_thickness() as f32;
            let size = size - border_thickness;
            let rect =
                graphene::Rect::new(border_thickness / 2.0, border_thickness / 2.0, size, size);

            snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, size / 2.0));
            snapshot.append_color(&gdk::RGBA::GREEN, &rect);
            snapshot.pop();
            snapshot.pop();

            snapshot.append_node(&child_snapshot.to_node().unwrap());
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    pub(crate) struct CircularProgressBar(ObjectSubclass<imp::CircularProgressBar>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for CircularProgressBar {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl CircularProgressBar {
    pub(crate) fn percentage(&self) -> f64 {
        self.imp().percentage.get()
    }

    pub(crate) fn set_percentage(&self, value: f64) {
        let value = (value.clamp(0.0, 1.0) * 100.0).round() / 100.0;

        if self.percentage() != value {
            self.imp().percentage.set(value);
            self.queue_draw();
            self.notify("percentage");
        }
    }

    pub(crate) fn border_thickness(&self) -> u32 {
        self.imp().border_thickness.get()
    }

    pub(crate) fn set_border_thickness(&self, value: u32) {
        if self.border_thickness() != value {
            self.imp().border_thickness.set(value);
            self.queue_draw();
            self.notify("border-thickness");
        }
    }
}
