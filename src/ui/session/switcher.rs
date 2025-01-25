use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use glib::Properties;
use gtk::glib;
use gtk::CompositeTemplate;

use crate::model;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::Switcher)]
    #[template(resource = "/app/drey/paper-plane/ui/session/switcher.ui")]
    pub(crate) struct Switcher {
        #[property(get, set, construct, nullable)]
        pub(super) client_manager: glib::WeakRef<model::ClientManager>,
        #[template_child]
        pub(super) popover_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub(super) add_session_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) signal_list_item_factory: TemplateChild<gtk::SignalListItemFactory>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Switcher {
        const NAME: &'static str = "PaplSessionSwitcher";
        type Type = super::Switcher;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            klass.set_css_name("sessionswitcher");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Switcher {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.popover_menu.set_parent(&self.add_session_button.get());
        }

        fn dispose(&self) {
            self.popover_menu.unparent();
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for Switcher {
        fn root(&self) {
            self.parent_root();

            let obj = &*self.obj();
            let client_manager_view = obj.client_manager_view();

            self.set_selected(&client_manager_view);
            client_manager_view.connect_active_client_notify(clone!(
                #[weak]
                obj,
                move |manager| {
                    obj.imp().set_selected(manager);
                }
            ));
        }
    }

    #[gtk::template_callbacks]
    impl Switcher {
        fn set_selected(&self, client_manager_view: &ui::ClientManagerView) {
            let position = self
                .selection
                .model()
                .unwrap()
                .iter::<glib::Object>()
                .position(|client| client.ok() == client_manager_view.active_client().and_upcast());

            self.selection.set_selected(
                position
                    .map(|pos| pos as u32)
                    .unwrap_or(gtk::INVALID_LIST_POSITION),
            );
        }

        #[template_callback]
        fn on_notify_client_manager(&self) {
            let obj = &*self.obj();

            let filter = gtk::CustomFilter::new(|item| {
                item.downcast_ref::<model::Client>()
                    .unwrap()
                    .state()
                    .and_downcast::<model::ClientStateSession>()
                    .is_some()
            });

            let filter_list_model =
                gtk::FilterListModel::new(obj.client_manager(), Some(filter.clone()));

            if let Some(client_manager) = obj.client_manager() {
                client_manager.connect_client_logged_in(clone!(
                    #[weak]
                    filter,
                    move |_, _| {
                        filter.changed(gtk::FilterChange::Different);
                    }
                ));
            }

            self.selection.set_model(Some(&filter_list_model));
        }

        #[template_callback]
        fn on_add_session_button_long_pressed(&self) {
            self.popover_menu.popup();
        }

        #[template_callback]
        fn on_list_view_activated(&self, position: u32) {
            self.selection.select_item(position, true);

            let client = self
                .selection
                .selected_item()
                .unwrap()
                .downcast::<model::Client>()
                .unwrap();

            self.obj().client_manager_view().set_active_client(&client);
        }

        #[template_callback]
        fn on_signal_list_item_factory_bind(&self, list_item: &glib::Object) {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            list_item.set_selectable(false);

            let session = list_item
                .item()
                .and_downcast::<model::Client>()
                .and_then(|client| client.state())
                .and_downcast::<model::ClientStateSession>()
                .unwrap();

            list_item.set_child(Some(
                &adw::Bin::builder()
                    .child(&ui::SessionRow::new(&session))
                    .margin_top(6)
                    .margin_bottom(6)
                    .build(),
            ));
        }

        #[template_callback]
        fn on_signal_list_item_factory_unbind(&self, list_item: &glib::Object) {
            list_item
                .downcast_ref::<gtk::ListItem>()
                .unwrap()
                .set_child(gtk::Widget::NONE);
        }
    }
}

glib::wrapper! {
    pub(crate) struct Switcher(ObjectSubclass<imp::Switcher>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Switcher {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl Switcher {
    pub(crate) fn client_manager_view(&self) -> ui::ClientManagerView {
        utils::ancestor::<_, ui::ClientManagerView>(self)
    }
}
