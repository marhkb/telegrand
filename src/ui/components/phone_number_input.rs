use std::borrow::Cow;
use std::cell::Cell;
use std::cell::OnceCell;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use adw::prelude::*;
use gettextrs::gettext;
use glib::clone;
use gtk::glib;
use gtk::glib::subclass::Signal;
use gtk::pango;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/components/phone_number_input.ui")]
    pub(crate) struct PhoneNumberInput {
        /// The system's country code
        pub(super) system_country_code: OnceCell<Option<String>>,
        /// Tuple, that keeps track of the calling code's start and end position within the entered
        /// phone number. These bounds are used to replace and highlight the current calling code
        /// and to automatically select the number without calling code in case Telegram complains
        /// about an invalid number.
        pub(super) calling_code_bounds: Cell<(usize, usize)>,
        #[template_child]
        pub(super) list_box: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) combo_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) entry_row: TemplateChild<adw::EntryRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PhoneNumberInput {
        const NAME: &'static str = "PaplPhoneNumberInput";
        type Type = super::PhoneNumberInput;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Editable,);

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PhoneNumberInput {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![Signal::builder("activate").build()])
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecObject::builder::<model::CountryList>("model")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecString::builder("number")
                        .explicit_notify()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "model" => obj.set_model(value.get().unwrap()),
                "number" => {
                    obj.set_number(&value.get::<Option<String>>().unwrap().unwrap_or_default())
                }
                other => self.entry_row.set_property(other, value),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();

            match pspec.name() {
                "model" => obj.model().to_value(),
                "number" => obj.number().to_value(),
                other => self.entry_row.property(other),
            }
        }
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Set the system country code.
            self.system_country_code
                .set(
                    locale_config::Locale::current()
                        .as_ref()
                        .split(',')
                        .next()
                        .and_then(|lang_country| lang_country.split('-').nth(1))
                        .map(str::to_string),
                )
                .unwrap();

            // Show the `model::CountryInfo`'s name in the combo box.
            self.combo_row
                .set_expression(Some(&model::CountryInfo::this_expression("name")));

            // We need these two handlers to block signals. Otherwise, the application would hang up
            // when the user enters a number or selects a country from the combo box.
            let combo_row_handler = Rc::new(RefCell::new(None));
            let entry_handler = Rc::new(RefCell::new(None));

            combo_row_handler.replace(Some(self.combo_row.connect_selected_item_notify(clone!(
                #[weak]
                obj,
                #[strong]
                entry_handler,
                move |combo_row| {
                    combo_row.set_subtitle("");

                    let imp = obj.imp();

                    let entry_handler = entry_handler.borrow();
                    let entry_handler = entry_handler.as_ref().unwrap();

                    let (pos_start, pos_end) = imp.calling_code_bounds.get();
                    let number_prefix = (0..pos_start)
                        .map(|_| ' ')
                        .chain(Some('+'))
                        .collect::<String>();

                    let (number, pos_start, pos_end) = obj
                        .selected_country_info()
                        .map(|selected_country_info| {
                            let calling_codes = selected_country_info.calling_codes();
                            let first_calling_code = calling_codes.first_or_empty();
                            (
                                [
                                    number_prefix.as_str(),
                                    first_calling_code,
                                    obj.number()
                                        .chars()
                                        .skip(pos_end)
                                        .collect::<String>()
                                        .as_str(),
                                ]
                                .concat(),
                                pos_start,
                                pos_start + first_calling_code.chars().count() + 1,
                            )
                        })
                        .unwrap_or((number_prefix, 0, 1));

                    imp.entry_row.block_signal(entry_handler);
                    obj.set_number(&number);
                    imp.entry_row.unblock_signal(entry_handler);

                    imp.calling_code_bounds.set((pos_start, pos_end));

                    obj.highlight_calling_code();
                }
            ))));

            // Format the phone number and reset the cursor.
            entry_handler.replace(Some(self.entry_row.connect_changed(clone!(
                #[weak]
                obj,
                #[strong]
                combo_row_handler,
                move |_| {
                    if let Some(ref model) = obj.model() {
                        let imp = obj.imp();

                        let combo_row_handler = combo_row_handler.borrow();
                        let combo_row_handler = combo_row_handler.as_ref().unwrap();

                        let number = obj.number();
                        let (text_pos_start, text_pos_end, list_pos) =
                            match number.char_indices().find(|(_, c)| !c.is_whitespace()) {
                                Some((text_pos_start, '+')) => {
                                    let analysis = model.analyze_for_calling_code(
                                        number
                                            .chars()
                                            .skip(text_pos_start + 1)
                                            .collect::<String>()
                                            .trim_end(),
                                        obj.preferred_country_code().as_deref(),
                                    );
                                    (
                                        text_pos_start,
                                        text_pos_start as u32 + analysis.code_len + 1,
                                        analysis.list_pos,
                                    )
                                }
                                _ => (0, 0, None),
                            };

                        imp.combo_row.block_signal(combo_row_handler);
                        obj.set_selected_country_code(list_pos);
                        imp.combo_row.unblock_signal(combo_row_handler);

                        imp.calling_code_bounds
                            .set((text_pos_start, text_pos_end as usize));

                        obj.highlight_calling_code();
                    }
                }
            ))));

            // We give focus the the phone number entry as soon as the user has selected an country
            // from the combo box.
            let focus_events = gtk::EventControllerFocus::new();
            focus_events.connect_leave(clone!(
                #[weak]
                obj,
                move |_| {
                    // We need to set the cursor position at the end on the next idle.
                    glib::idle_add_local_once(clone!(
                        #[weak]
                        obj,
                        move || {
                            obj.imp().entry_row.set_position(i32::MAX);
                        }
                    ));
                }
            ));
            self.combo_row.add_controller(focus_events);
        }

        fn dispose(&self) {
            self.list_box.unparent();
        }
    }

    impl WidgetImpl for PhoneNumberInput {
        fn grab_focus(&self) -> bool {
            self.entry_row.grab_focus()
        }
    }

    impl EditableImpl for PhoneNumberInput {
        fn delegate(&self) -> Option<gtk::Editable> {
            self.entry_row.delegate()
        }
    }

    #[gtk::template_callbacks]
    impl PhoneNumberInput {
        #[template_callback]
        fn on_entry_row_activated(&self) {
            self.obj().emit_by_name::<()>("activate", &[]);
        }
    }
}

glib::wrapper! {
    pub(crate) struct PhoneNumberInput(ObjectSubclass<imp::PhoneNumberInput>)
        @extends gtk::Widget,
        @implements gtk::Editable, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PhoneNumberInput {
    pub(crate) fn model(&self) -> Option<model::CountryList> {
        self.imp()
            .combo_row
            .model()
            .map(|model| model.downcast::<model::CountryList>().unwrap())
    }

    pub(crate) fn set_model(&self, model: Option<&model::CountryList>) {
        if self.model().as_ref() == model {
            return;
        }
        self.imp().combo_row.set_model(model);
        self.notify("model");

        self.select_system_country_code();
    }

    pub(crate) fn number(&self) -> glib::GString {
        self.imp().entry_row.text()
    }

    pub(crate) fn set_number(&self, number: &str) {
        if self.number() == number {
            return;
        }
        self.imp().entry_row.set_text(number);
        self.notify("number");
    }

    /// Returns the currently selected `model::CountryInfo`.
    pub(crate) fn selected_country_info(&self) -> Option<model::CountryInfo> {
        self.imp()
            .combo_row
            .selected_item()
            .map(|item| item.downcast::<model::CountryInfo>().unwrap())
    }

    fn set_selected_country_code(&self, position: Option<u32>) {
        let combo_row = &self.imp().combo_row;

        combo_row.set_selected(position.unwrap_or(gtk::INVALID_LIST_POSITION));

        if position.is_some() {
            combo_row.set_subtitle("");
        } else {
            combo_row.set_subtitle(&gettext("You entered an invalid country code."));
        }
    }

    /// Performs a text selection of the whole number but leaves out the calling code.
    pub(crate) fn select_number_without_calling_code(&self) {
        let imp = self.imp();
        imp.entry_row.grab_focus();
        imp.entry_row
            .select_region(imp.calling_code_bounds.get().1 as i32, i32::MAX);
    }

    /// Function to determine the preferred country code. This is either the code of the currently
    /// selected country or the system country code.
    ///
    /// The functions prefers the code of the currently selected country over the system country
    /// code if both share the same calling code.
    fn preferred_country_code(&self) -> Option<Cow<str>> {
        let system_country_code = self.imp().system_country_code.get().unwrap().as_deref();

        self.model()
            .and_then(|model| {
                self.selected_country_info()
                    .as_ref()
                    .map(model::CountryInfo::country_code)
                    .and_then(|country_code_1| {
                        let has_same_calling_code = system_country_code
                            .map(|country_code_2| {
                                model.same_calling_code(&country_code_1, country_code_2)
                            })
                            .unwrap_or_default();

                        if has_same_calling_code {
                            Some(Cow::Owned(country_code_1.to_owned()))
                        } else {
                            None
                        }
                    })
            })
            .or_else(|| system_country_code.map(Cow::Borrowed))
    }

    /// Sets the selected item of the the combo box to that one with the system country code.
    fn select_system_country_code(&self) {
        if let Some(model) = self.model() {
            let imp = self.imp();

            let position = imp
                .system_country_code
                .get()
                .unwrap()
                .as_deref()
                .and_then(|country| model.country_code_pos(country));

            self.set_selected_country_code(position);
        }
    }

    /// Highlights the calling code from the rest of the number.
    fn highlight_calling_code(&self) {
        let imp = self.imp();

        let attr_list = pango::AttrList::new();

        let (pos_start, pos_end) = imp.calling_code_bounds.get();
        if pos_start < pos_end {
            let mut attr = pango::AttrInt::new_weight(pango::Weight::Bold);
            attr.set_start_index(pos_start as u32);
            attr.set_end_index(pos_end as u32);
            attr_list.insert(attr);

            let mut attr = pango::AttrInt::new_foreground_alpha(u16::MAX / 2);
            attr.set_start_index(pos_start as u32);
            attr.set_end_index(pos_end as u32);
            attr_list.insert(attr);
        }

        imp.entry_row.set_attributes(Some(&attr_list));
    }
}
