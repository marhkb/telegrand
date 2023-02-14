use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};

use crate::session::sidebar::search::SectionType;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(string = r#"
    <interface>
      <template class="SidebarSearchSectionRow" parent="GtkWidget">
        <child>
          <object class="GtkInscription" id="label">
            <property name="hexpand">True</property>
            <property name="text-overflow">ellipsize-end</property>
            <style>
              <class name="heading"/>
            </style>
          </object>
        </child>
      </template>
    </interface>
    "#)]
    pub(crate) struct SectionRow {
        pub(super) section_type: Cell<SectionType>,
        pub(super) suffix: RefCell<Option<gtk::Widget>>,
        #[template_child]
        pub(super) label: TemplateChild<gtk::Inscription>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SectionRow {
        const NAME: &'static str = "SidebarSearchSectionRow";
        type Type = super::SectionRow;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.set_layout_manager_type::<gtk::BoxLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SectionRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecEnum::builder::<SectionType>("section-type")
                    .explicit_notify()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "section-type" => obj.set_section_type(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "section-type" => obj.section_type().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().update_content();
        }

        fn dispose(&self) {
            self.label.unparent();
            if let Some(suffix) = self.suffix.take() {
                suffix.unparent();
            }
        }
    }

    impl WidgetImpl for SectionRow {}
}

glib::wrapper! {
    pub(crate) struct SectionRow(ObjectSubclass<imp::SectionRow>)
        @extends gtk::Widget;
}

impl SectionRow {
    pub(crate) fn new(section_type: SectionType) -> Self {
        glib::Object::builder()
            .property("section-type", section_type)
            .build()
    }

    pub(crate) fn section_type(&self) -> SectionType {
        self.imp().section_type.get()
    }

    pub(crate) fn set_section_type(&self, section_type: SectionType) {
        if self.section_type() == section_type {
            return;
        }

        self.imp().section_type.set(section_type);

        self.update_content();

        self.notify("section-type");
    }

    fn update_content(&self) {
        let imp = self.imp();

        if let Some(suffix) = imp.suffix.take() {
            suffix.unparent();
        }

        match self.section_type() {
            SectionType::Chats => {
                imp.label.set_text(Some(&gettext("Chats")));
            }
            SectionType::Global => {
                imp.label.set_text(Some(&gettext("Global Search")));
            }
            SectionType::Recent => {
                imp.label.set_text(Some(&gettext("Recent")));

                let button = gtk::Button::builder()
                    .icon_name("clear-symbolic")
                    .action_name("sidebar-search.clear-recent-chats")
                    .build();
                button.add_css_class("flat");
                button.insert_before(self, gtk::Widget::NONE);
                imp.suffix.replace(Some(button.upcast()));
            }
        }
    }
}
