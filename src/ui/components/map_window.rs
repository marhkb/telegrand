use std::cell::RefCell;
use std::cell::RefMut;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::CompositeTemplate;
use shumate::prelude::*;

use crate::ui;

const ANIMATION_DURATION: u32 = 400;
const DEFAULT_ZOOM_LEVEL: f64 = 16.0;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/components/map_window.ui")]
    pub(crate) struct MapWindow {
        pub(super) center_marker_animations: [RefCell<Option<adw::TimedAnimation>>; 3],
        pub(super) zoom_animation: RefCell<Option<adw::TimedAnimation>>,
        #[template_child]
        pub(super) map: TemplateChild<ui::Map>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MapWindow {
        const NAME: &'static str = "PaplMapWindow";
        type Type = super::MapWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action("map-window.center-marker", None, move |widget, _, _| {
                widget.animate_center_marker();
            });

            klass.install_action("map-window.zoom-in", None, move |widget, _, _| {
                widget.zoom_in();
            });
            klass.install_action("map-window.zoom-out", None, move |widget, _, _| {
                widget.zoom_out();
            });

            klass.install_action("map-window.open", None, move |widget, _, _| {
                widget.open();
            });

            klass.install_action("map-window.close", None, move |widget, _, _| {
                widget.close();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MapWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            let viewport = obj.map().viewport();
            viewport.connect_zoom_level_notify(clone!(
                #[weak]
                obj,
                move |viewport| {
                    obj.update_zoom_actions(viewport);
                    obj.update_center_marker_animations_action(viewport);
                }
            ));
            viewport.connect_latitude_notify(clone!(
                #[weak]
                obj,
                move |viewport| {
                    obj.update_center_marker_animations_action(viewport);
                }
            ));
            viewport.connect_longitude_notify(clone!(
                #[weak]
                obj,
                move |viewport| {
                    obj.update_center_marker_animations_action(viewport);
                }
            ));
            obj.update_zoom_actions(&viewport);
        }
    }

    impl WidgetImpl for MapWindow {}
    impl WindowImpl for MapWindow {}
    impl AdwWindowImpl for MapWindow {}

    #[gtk::template_callbacks]
    impl MapWindow {
        #[template_callback]
        fn on_key_pressed(
            &self,
            key: gdk::Key,
            _: u32,
            modifier: gdk::ModifierType,
            _: &gtk::EventControllerKey,
        ) -> glib::Propagation {
            if key == gdk::Key::Escape
                || (key == gdk::Key::w && modifier == gdk::ModifierType::CONTROL_MASK)
            {
                self.obj().close();
            }

            glib::Propagation::Proceed
        }
    }
}

glib::wrapper! {
    pub(crate) struct MapWindow(ObjectSubclass<imp::MapWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

impl MapWindow {
    pub(crate) fn new(parent: Option<&gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    pub(crate) fn open(&self) {
        let (lat, lon) = self.imp().map.marker_location();

        gtk::UriLauncher::new(&format!(
            "https://www.openstreetmap.org/?mlat={lat}&mlon={lon}"
        ))
        .launch(gtk::Window::NONE, gio::Cancellable::NONE, |_| {});
    }

    pub(crate) fn map(&self) -> ui::Map {
        self.imp().map.get()
    }

    pub(crate) fn center_marker(&self) {
        self.map().center_marker(DEFAULT_ZOOM_LEVEL);
    }

    pub(crate) fn animate_center_marker(&self) {
        if !self.are_center_marker_animations_finished() {
            return;
        }

        self.action_set_enabled("map-window.center-marker", false);

        let imp = self.imp();

        let map = self.map();
        let viewport = map.viewport();
        let (lat, lon) = map.marker_location();

        let target_zoom = adw::PropertyAnimationTarget::new(&viewport, "zoom-level");
        let animation_zoom = adw::TimedAnimation::builder()
            .widget(self)
            .duration(ANIMATION_DURATION)
            .target(&target_zoom)
            .value_from(viewport.zoom_level())
            .value_to(DEFAULT_ZOOM_LEVEL)
            .build();

        let target_lat = adw::PropertyAnimationTarget::new(&viewport, "latitude");
        let animation_lat = adw::TimedAnimation::builder()
            .widget(self)
            .duration(ANIMATION_DURATION)
            .target(&target_lat)
            .value_from(viewport.latitude())
            .value_to(lat)
            .build();

        let target_lon = adw::PropertyAnimationTarget::new(&viewport, "longitude");
        let animation_lon = adw::TimedAnimation::builder()
            .widget(self)
            .duration(ANIMATION_DURATION)
            .target(&target_lon)
            .value_from(viewport.longitude())
            .value_to(lon)
            .build();

        animation_zoom.connect_state_notify(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_| {
                obj.update_center_marker_animations(
                    obj.imp().center_marker_animations[0].borrow_mut(),
                );
            }
        ));
        animation_lat.connect_state_notify(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_| {
                obj.update_center_marker_animations(
                    obj.imp().center_marker_animations[1].borrow_mut(),
                );
            }
        ));
        animation_lon.connect_state_notify(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_| {
                obj.update_center_marker_animations(
                    obj.imp().center_marker_animations[2].borrow_mut(),
                );
            }
        ));

        map.set_interactive(false);

        animation_zoom.play();
        animation_lat.play();
        animation_lon.play();

        imp.center_marker_animations[0].replace(Some(animation_zoom));
        imp.center_marker_animations[1].replace(Some(animation_lat));
        imp.center_marker_animations[2].replace(Some(animation_lon));
    }

    fn update_center_marker_animations(
        &self,
        mut animation_ref: RefMut<Option<adw::TimedAnimation>>,
    ) {
        if let Some(animation) = animation_ref.as_ref() {
            if animation.state() == adw::AnimationState::Finished {
                *animation_ref = None;
            }

            drop(animation_ref);
            self.update_center_marker_animations_action(&self.imp().map.viewport());
        }
    }

    fn update_center_marker_animations_action(&self, viewport: &shumate::Viewport) {
        let map = self.imp().map.get();
        let finished = self.are_center_marker_animations_finished();
        map.set_interactive(finished);

        self.action_set_enabled(
            "map-window.center-marker",
            (viewport.zoom_level() != DEFAULT_ZOOM_LEVEL
                || (viewport.latitude(), viewport.longitude()) != map.marker_location())
                && finished,
        );
    }

    fn are_center_marker_animations_finished(&self) -> bool {
        self.imp()
            .center_marker_animations
            .iter()
            .all(|animation| animation.borrow().is_none())
    }

    pub(crate) fn zoom_in(&self) {
        self.animate_zoom(1.0);
    }

    pub(crate) fn zoom_out(&self) {
        self.animate_zoom(-1.0);
    }

    pub(crate) fn animate_zoom(&self, diff: f64) {
        let imp = self.imp();

        if let Some(animation) = imp.zoom_animation.take() {
            animation.skip();
        }

        let viewport = self.map().viewport();

        let target = adw::PropertyAnimationTarget::new(&viewport, "zoom-level");
        let animation = adw::TimedAnimation::builder()
            .widget(self)
            .duration(ANIMATION_DURATION)
            .target(&target)
            .value_from(viewport.zoom_level())
            .value_to((viewport.zoom_level() + diff).clamp(
                viewport.min_zoom_level() as f64,
                viewport.max_zoom_level() as f64,
            ))
            .build();
        animation.play();

        imp.zoom_animation.replace(Some(animation));
    }

    fn update_zoom_actions(&self, viewport: &shumate::Viewport) {
        self.action_set_enabled(
            "map-window.zoom-in",
            viewport.zoom_level() < viewport.max_zoom_level() as f64,
        );
        self.action_set_enabled(
            "map-window.zoom-out",
            viewport.zoom_level() > viewport.min_zoom_level() as f64,
        );
    }
}
