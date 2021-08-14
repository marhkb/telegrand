use gettextrs::gettext;
use gtk::glib;
use once_cell::sync::Lazy;
use regex::Regex;
use std::future::Future;
use tdgrand::enums::MessageContent as TelegramMessageContent;

use crate::RUNTIME;

pub static PROTOCOL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\w+://").unwrap());

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn linkify(text: &str) -> String {
    if !PROTOCOL_RE.is_match(text) {
        format!("http://{}", text)
    } else {
        text.to_string()
    }
}

pub fn stringify_message_content(content: TelegramMessageContent, use_markup: bool) -> String {
    match content {
        TelegramMessageContent::MessageText(content) => content.text.text,
        _ => {
            let text = gettext("Unsupported message");
            if use_markup {
                format!("<i>{}</i>", text)
            } else {
                text
            }
        }
    }
}

// Function from https://gitlab.gnome.org/GNOME/fractal/-/blob/fractal-next/src/utils.rs
pub fn do_async<
    R: Send + 'static,
    F1: Future<Output = R> + Send + 'static,
    F2: Future<Output = ()> + 'static,
    FN: FnOnce(R) -> F2 + 'static,
>(
    priority: glib::source::Priority,
    tokio_fut: F1,
    glib_closure: FN,
) {
    let (sender, receiver) = tokio::sync::oneshot::channel();

    glib::MainContext::default().spawn_local_with_priority(priority, async move {
        glib_closure(receiver.await.unwrap()).await
    });

    RUNTIME.spawn(async move { sender.send(tokio_fut.await) });
}
