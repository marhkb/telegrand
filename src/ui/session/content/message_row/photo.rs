use std::cell::RefCell;
use std::sync::OnceLock;

use glib::clone;
use glib::closure;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::model;
use crate::types::MessageId;
use crate::ui;
use crate::ui::MessageBaseExt;
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/app/drey/paper-plane/ui/session/content/message_row/photo.ui")]
    pub(crate) struct MessagePhoto {
        pub(super) binding: RefCell<Option<gtk::ExpressionWatch>>,
        pub(super) handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub(super) message: glib::WeakRef<model::Message>,
        #[template_child]
        pub(super) message_bubble: TemplateChild<ui::MessageBubble>,
        #[template_child]
        pub(super) picture: TemplateChild<ui::MediaPicture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessagePhoto {
        const NAME: &'static str = "PaplMessagePhoto";
        type Type = super::MessagePhoto;
        type ParentType = ui::MessageBase;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessagePhoto {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![glib::ParamSpecObject::builder::<model::Message>("message")
                    .explicit_notify()
                    .build()]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "message" => obj.set_message(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "message" => self.message.upgrade().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().connect_scale_factor_notify(|obj| {
                obj.update_photo(&obj.imp().message.upgrade().unwrap());
            });
        }
    }

    impl WidgetImpl for MessagePhoto {}
    impl ui::MessageBaseImpl for MessagePhoto {}
}

glib::wrapper! {
    pub(crate) struct MessagePhoto(ObjectSubclass<imp::MessagePhoto>)
        @extends gtk::Widget, ui::MessageBase;
}

impl ui::MessageBaseExt for MessagePhoto {
    type Message = model::Message;

    fn set_message(&self, message: &Self::Message) {
        let imp = self.imp();

        let old_message = imp.message.upgrade();
        if old_message.as_ref() == Some(message) {
            return;
        }

        if let Some(binding) = imp.binding.take() {
            binding.unwatch();
        }

        if let Some(old_message) = old_message {
            let handler_id = imp.handler_id.take().unwrap();
            old_message.disconnect(handler_id);
        }

        imp.message.set(Some(message));

        imp.message_bubble.update_from_message(message, true);

        // Setup caption expression
        let caption_binding = model::Message::this_expression("content")
            .chain_closure::<String>(closure!(
                |_: model::Message, content: model::BoxedMessageContent| {
                    if let tdlib::enums::MessageContent::MessagePhoto(data) = content.0 {
                        utils::parse_formatted_text(data.caption)
                    } else {
                        unreachable!();
                    }
                }
            ))
            .bind(&*imp.message_bubble, "label", Some(message));
        imp.binding.replace(Some(caption_binding));

        // Load photo
        let handler_id = message.connect_content_notify(clone!(
            #[weak(rename_to = obj)]
            self,
            move |message| {
                obj.update_photo(message);
            }
        ));
        imp.handler_id.replace(Some(handler_id));
        self.update_photo(message);

        self.notify("message");
    }
}

impl MessagePhoto {
    fn message_id(&self) -> Option<MessageId> {
        self.imp().message.upgrade().map(|message| message.id())
    }

    fn update_photo(&self, message: &model::Message) {
        if let tdlib::enums::MessageContent::MessagePhoto(mut data) = message.content().0 {
            let imp = self.imp();
            // Choose the right photo size based on the screen scale factor.
            // See https://core.telegram.org/api/files#image-thumbnail-types for more
            // information about photo sizes.
            let photo_size = if self.scale_factor() > 2 {
                data.photo.sizes.pop().unwrap()
            } else {
                let type_ = if self.scale_factor() > 1 { "y" } else { "x" };

                match data.photo.sizes.iter().position(|s| s.r#type == type_) {
                    Some(pos) => data.photo.sizes.swap_remove(pos),
                    None => data.photo.sizes.pop().unwrap(),
                }
            };

            imp.picture
                .set_aspect_ratio(photo_size.width as f64 / photo_size.height as f64);

            if photo_size.photo.local.is_downloading_completed {
                self.load_photo(photo_size.photo.local.path);
            } else {
                imp.picture.set_paintable(
                    data.photo
                        .minithumbnail
                        .and_then(|m| {
                            gdk::Texture::from_bytes(&glib::Bytes::from_owned(glib::base64_decode(
                                &m.data,
                            )))
                            .ok()
                        })
                        .as_ref(),
                );

                let file_id = photo_size.photo.id;
                let session = message.chat_().session_();
                utils::spawn(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    #[weak]
                    session,
                    async move {
                        obj.download_photo(file_id, &session).await;
                    }
                ));
            }
        }
    }

    async fn download_photo(&self, file_id: i32, session: &model::ClientStateSession) {
        match session.download_file(file_id).await {
            Ok(file) => {
                self.load_photo(file.local.path);
            }
            Err(e) => {
                log::warn!("Failed to download a photo: {e:?}");
            }
        }
    }

    fn load_photo(&self, path: String) {
        if let Some(message_id) = self.message_id() {
            utils::spawn(clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    let result = gio::spawn_blocking(move || utils::decode_image_from_path(&path))
                        .await
                        .unwrap();

                    // Check if the current message id is the same as the one at
                    // the time of the request. It may be changed because of the
                    // ListView recycling while decoding the image. It may also
                    // that the message has been already removed from the history
                    // and the WeakRef is None (after successful sent)
                    if obj.message_id().filter(|id| *id == message_id).is_some() {
                        match result {
                            Ok(texture) => {
                                obj.imp().picture.set_paintable(Some(&texture));
                            }
                            Err(e) => {
                                log::warn!("Error decoding a photo: {e:?}");
                            }
                        }
                    }
                }
            ));
        }
    }
}
