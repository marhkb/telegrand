use std::cell::OnceCell;
use std::cell::RefCell;

use adw::subclass::prelude::*;
use glib::clone;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::ui;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/send_media_window.ui")]
    pub(crate) struct SendMediaWindow {
        pub(super) chat: glib::WeakRef<model::Chat>,
        pub(super) path: OnceCell<String>,
        pub(super) reply_to: OnceCell<i64>,
        pub(super) emoji_chooser: RefCell<Option<gtk::EmojiChooser>>,
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub(super) caption_entry: TemplateChild<ui::MessageEntry>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SendMediaWindow {
        const NAME: &'static str = "PaplSendMediaWindow";
        type Type = super::SendMediaWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action_async(
                "send-media-window.send-message",
                None,
                |widget, _, _| async move {
                    widget.send_message(false).await;
                },
            );
            klass.install_action_async(
                "send-media-window.send-as-file",
                None,
                |widget, _, _| async move {
                    widget.send_message(true).await;
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SendMediaWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.caption_entry.connect_activate(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.activate_action("send-media-window.send-message", None)
                        .unwrap()
                }
            ));

            self.caption_entry.connect_emoji_button_press(clone!(
                #[weak]
                obj,
                move |_, button| {
                    obj.show_emoji_chooser(&button);
                }
            ));
        }

        fn dispose(&self) {
            if let Some(emoji_chooser) = self.emoji_chooser.take() {
                emoji_chooser.unparent();
            }
        }
    }

    impl WidgetImpl for SendMediaWindow {}
    impl WindowImpl for SendMediaWindow {}
    impl AdwWindowImpl for SendMediaWindow {}

    #[gtk::template_callbacks]
    impl SendMediaWindow {
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
    pub(crate) struct SendMediaWindow(ObjectSubclass<imp::SendMediaWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

impl SendMediaWindow {
    pub(crate) fn new(
        parent: &gtk::Window,
        chat: &model::Chat,
        path: String,
        reply_to: i64,
    ) -> Self {
        let obj: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();
        let imp = obj.imp();

        imp.picture.set_filename(Some(&path));
        imp.caption_entry.set_chat(Some(chat.clone()));

        imp.chat.set(Some(chat));
        imp.path.set(path).unwrap();
        imp.reply_to.set(reply_to).unwrap();

        obj
    }

    fn show_emoji_chooser(&self, parent: &impl IsA<gtk::Widget>) {
        let imp = self.imp();
        let mut emoji_chooser = imp.emoji_chooser.borrow_mut();
        if emoji_chooser.is_none() {
            let chooser = gtk::EmojiChooser::new();
            chooser.set_parent(parent);
            chooser.connect_emoji_picked(clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, emoji| {
                    obj.imp().caption_entry.insert_at_cursor(emoji);
                }
            ));
            chooser.connect_hide(clone!(
                #[weak(rename_to = obj)]
                self,
                move |_| {
                    obj.imp().caption_entry.grab_focus();
                }
            ));
            *emoji_chooser = Some(chooser);
        }
        emoji_chooser.as_ref().unwrap().popup();
    }

    async fn send_message(&self, send_as_file: bool) {
        use tdlib::enums::*;
        use tdlib::types::*;

        let imp = self.imp();

        let path = imp.path.get().unwrap().clone();
        let caption = imp.caption_entry.as_markdown().await;
        let file = InputFile::Local(InputFileLocal { path });

        let content = if send_as_file {
            InputMessageContent::InputMessageDocument(InputMessageDocument {
                document: file,
                thumbnail: None,
                disable_content_type_detection: true,
                caption,
            })
        } else {
            let paintable = imp.picture.paintable().unwrap();

            InputMessageContent::InputMessagePhoto(InputMessagePhoto {
                photo: file,
                thumbnail: None,
                added_sticker_file_ids: vec![],
                width: paintable.intrinsic_width(),
                height: paintable.intrinsic_height(),
                caption,
                self_destruct_type: None,
                has_spoiler: false,
            })
        };

        let chat = imp.chat.upgrade().unwrap();
        let chat_id = chat.id();
        let client_id = chat.session_().client_().id();

        let reply_to = Some(MessageReplyTo::Message(MessageReplyToMessage {
            chat_id,
            message_id: *imp.reply_to.get().unwrap(),
        }));

        match tdlib::functions::send_message(chat_id, 0, reply_to, None, content, client_id).await {
            Ok(_) => self.close(),
            Err(e) => imp.toast_overlay.add_toast(
                adw::Toast::builder()
                    .title(e.message)
                    .timeout(3)
                    .priority(adw::ToastPriority::High)
                    .build(),
            ),
        }
    }
}
