mod avatar;
mod chat_folder;
mod chat_list;
mod mini_thumbnail;
mod row;
mod search;
mod selection;

use std::cell::Cell;
use std::cell::OnceCell;

use glib::clone;
use glib::closure;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

pub(crate) use self::avatar::Avatar;
pub(crate) use self::chat_folder::Bar as ChatFolderBar;
pub(crate) use self::chat_folder::Icon as ChatFolderIcon;
pub(crate) use self::chat_folder::Row as ChatFolderRow;
pub(crate) use self::chat_folder::Selection as ChatFolderSelection;
pub(crate) use self::chat_list::ChatList;
pub(crate) use self::mini_thumbnail::MiniThumbnail;
pub(crate) use self::row::Row;
pub(crate) use self::search::ItemRow as SearchItemRow;
pub(crate) use self::search::Row as SearchRow;
pub(crate) use self::search::Search;
pub(crate) use self::search::Section as SearchSection;
pub(crate) use self::search::SectionRow as SearchSectionRow;
pub(crate) use self::search::SectionType as SearchSectionType;
pub(crate) use self::selection::Selection;
use crate::model;
use crate::ui;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::Sidebar)]
    #[template(resource = "/app/drey/paper-plane/ui/session/sidebar/mod.ui")]
    pub(crate) struct Sidebar {
        #[property(get, set)]
        pub(super) compact: Cell<bool>,
        #[property(get, set, nullable)]
        pub(super) selected_chat: glib::WeakRef<model::Chat>,
        #[property(get, set = Self::set_session, explicit_notify)]
        pub(super) session: glib::WeakRef<model::ClientStateSession>,
        pub(super) row_menu: OnceCell<gtk::PopoverMenu>,
        #[template_child]
        pub(super) navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub(super) main_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub(super) search: TemplateChild<Search>,
        #[template_child]
        pub(super) title_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) archive_unread_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) snow: TemplateChild<ui::Snow>,
        #[template_child]
        pub(super) folder_bar: TemplateChild<ui::SidebarChatFolderBar>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Sidebar {
        const NAME: &'static str = "PaplSidebar";
        type Type = super::Sidebar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action("sidebar.show-sessions", None, move |widget, _, _| {
                widget.show_sessions();
            });

            klass.install_action("sidebar.start-search", None, move |widget, _, _| {
                widget.begin_chats_search();
            });

            klass.install_action("sidebar.show-archived-chats", None, move |widget, _, _| {
                widget.show_archived_chats();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl Sidebar {
        fn set_session(&self, session: Option<&model::ClientStateSession>) {
            let obj = &*self.obj();
            if obj.session().as_ref() == session {
                return;
            }

            if let Some(session) = session {
                session.archive_chat_list().connect_items_changed(clone!(
                    #[weak]
                    obj,
                    move |chat_list, _, _, _| {
                        if chat_list.n_items() == 0 {
                            obj.imp().navigation_view.pop_to_tag("chats");
                        }
                    }
                ));
            }

            self.session.set(session);
            obj.notify_session();
        }

        #[template_callback]
        fn close_search(&self) {
            self.navigation_view.pop_to_tag("chats");
        }
    }

    impl ObjectImpl for Sidebar {
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

            let obj = &*self.obj();

            let session_expr = Self::Type::this_expression("session");

            session_expr
                .chain_property::<model::ClientStateSession>("chat-folder-list")
                .chain_property::<model::ChatFolderList>("has-folders")
                .bind(&self.folder_bar.get(), "visible", Some(obj));

            let archive_chat_list_expr =
                session_expr.chain_property::<model::ClientStateSession>("archive-chat-list");

            let archive_chat_list_unread_count_expr =
                archive_chat_list_expr.chain_property::<model::ChatList>("unread-chat-count");

            archive_chat_list_expr
                .chain_property::<model::ChatList>("len")
                .chain_closure::<String>(closure!(|_: Self::Type, len: u32| {
                    if len > 0 {
                        "archive-button"
                    } else {
                        "chats-label"
                    }
                }))
                .bind(&self.title_stack.get(), "visible-child-name", Some(obj));

            archive_chat_list_unread_count_expr.bind(
                &self.archive_unread_label.get(),
                "label",
                Some(obj),
            );
            archive_chat_list_unread_count_expr.bind(
                &self.archive_unread_label.get(),
                "visible",
                Some(obj),
            );
        }

        fn dispose(&self) {
            utils::unparent_children(&*self.obj());
        }
    }

    impl WidgetImpl for Sidebar {
        fn direction_changed(&self, previous_direction: gtk::TextDirection) {
            let obj = self.obj();

            if obj.direction() == previous_direction {
                return;
            }

            if let Some(menu) = self.row_menu.get() {
                menu.set_halign(if obj.direction() == gtk::TextDirection::Rtl {
                    gtk::Align::End
                } else {
                    gtk::Align::Start
                });
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct Sidebar(ObjectSubclass<imp::Sidebar>)
        @extends gtk::Widget;
}

impl Sidebar {
    pub(crate) fn row_menu(&self) -> &gtk::PopoverMenu {
        self.imp().row_menu.get_or_init(|| {
            let menu =
                gtk::Builder::from_resource("/app/drey/paper-plane/ui/session/sidebar/row_menu.ui")
                    .object::<gtk::PopoverMenu>("menu")
                    .unwrap();

            menu.set_halign(if self.direction() == gtk::TextDirection::Rtl {
                gtk::Align::End
            } else {
                gtk::Align::Start
            });

            menu
        })
    }

    pub(crate) fn show_sessions(&self) {
        self.imp().navigation_view.push_by_tag("sessions");
    }

    pub(crate) fn begin_chats_search(&self) {
        let imp = self.imp();
        imp.search.reset();
        imp.search.grab_focus();
        imp.navigation_view.push_by_tag("search");
    }

    pub(crate) fn show_archived_chats(&self) {
        self.imp().navigation_view.push_by_tag("archived-chats");
    }
}
