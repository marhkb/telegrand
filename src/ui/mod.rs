mod client_manager_view;
mod client_view;
mod components;
mod login;
mod session;
mod window;

use gtk::glib::prelude::*;

pub(crate) use self::client_manager_view::ClientManagerView;
pub(crate) use self::client_view::ClientView;
pub(crate) use self::components::AnimatedBin;
pub(crate) use self::components::Avatar;
pub(crate) use self::components::AvatarMapMarker;
pub(crate) use self::components::AvatarWithSelection;
pub(crate) use self::components::CircularProgressBar;
pub(crate) use self::components::IconMapMarker;
pub(crate) use self::components::Map;
pub(crate) use self::components::MapMarker;
pub(crate) use self::components::MapWindow;
pub(crate) use self::components::MessageEntry;
pub(crate) use self::components::PhoneNumberInput;
pub(crate) use self::components::Snow;
pub(crate) use self::components::Sticker;
pub(crate) use self::login::Code as LoginCode;
pub(crate) use self::login::Login;
pub(crate) use self::login::OtherDevice as LoginOtherDevice;
pub(crate) use self::login::Password as LoginPassword;
pub(crate) use self::login::PhoneNumber as LoginPhoneNumber;
pub(crate) use self::login::Registration as LoginRegistration;
pub(crate) use self::session::Background;
pub(crate) use self::session::ChatActionBar;
pub(crate) use self::session::ChatHistory;
pub(crate) use self::session::ChatHistoryRow;
pub(crate) use self::session::ChatInfoWindow;
pub(crate) use self::session::ContactRow;
pub(crate) use self::session::ContactsWindow;
pub(crate) use self::session::Content;
pub(crate) use self::session::EventRow;
pub(crate) use self::session::MediaPicture;
pub(crate) use self::session::MessageBase;
pub(crate) use self::session::MessageBaseExt;
pub(crate) use self::session::MessageBaseImpl;
pub(crate) use self::session::MessageBubble;
pub(crate) use self::session::MessageDocument;
pub(crate) use self::session::MessageDocumentStatusIndicator;
pub(crate) use self::session::MessageIndicators;
pub(crate) use self::session::MessageLabel;
pub(crate) use self::session::MessageLocation;
pub(crate) use self::session::MessagePhoto;
pub(crate) use self::session::MessageReply;
pub(crate) use self::session::MessageRow;
pub(crate) use self::session::MessageSticker;
pub(crate) use self::session::MessageText;
pub(crate) use self::session::MessageVenue;
pub(crate) use self::session::MessageVideo;
pub(crate) use self::session::PreferencesWindow;
pub(crate) use self::session::Row as SessionRow;
pub(crate) use self::session::SendMediaWindow;
pub(crate) use self::session::Session;
pub(crate) use self::session::Sidebar;
pub(crate) use self::session::SidebarAvatar;
pub(crate) use self::session::SidebarMiniThumbnail;
pub(crate) use self::session::SidebarRow;
pub(crate) use self::session::SidebarSearch;
pub(crate) use self::session::SidebarSearchItemRow;
pub(crate) use self::session::SidebarSearchRow;
pub(crate) use self::session::SidebarSearchSection;
pub(crate) use self::session::SidebarSearchSectionRow;
pub(crate) use self::session::SidebarSearchSectionType;
pub(crate) use self::session::Switcher as SessionSwitcher;
pub(crate) use self::window::Window;

pub(crate) fn init() {
    AnimatedBin::static_type();
    Avatar::static_type();
    AvatarMapMarker::static_type();
    AvatarWithSelection::static_type();
    Background::static_type();
    ChatActionBar::static_type();
    ChatHistory::static_type();
    ChatHistoryRow::static_type();
    ChatInfoWindow::static_type();
    CircularProgressBar::static_type();
    ClientManagerView::static_type();
    ClientView::static_type();
    ContactRow::static_type();
    ContactsWindow::static_type();
    Content::static_type();
    EventRow::static_type();
    IconMapMarker::static_type();
    Login::static_type();
    LoginCode::static_type();
    LoginOtherDevice::static_type();
    LoginPassword::static_type();
    LoginPhoneNumber::static_type();
    LoginRegistration::static_type();
    Map::static_type();
    MapMarker::static_type();
    MapWindow::static_type();
    MediaPicture::static_type();
    MessageBase::static_type();
    MessageBubble::static_type();
    MessageDocument::static_type();
    MessageDocumentStatusIndicator::static_type();
    MessageEntry::static_type();
    MessageIndicators::static_type();
    MessageLabel::static_type();
    MessageLocation::static_type();
    MessagePhoto::static_type();
    MessageReply::static_type();
    MessageRow::static_type();
    MessageSticker::static_type();
    MessageText::static_type();
    MessageVenue::static_type();
    MessageVideo::static_type();
    PhoneNumberInput::static_type();
    PreferencesWindow::static_type();
    SendMediaWindow::static_type();
    Session::static_type();
    SessionRow::static_type();
    SessionSwitcher::static_type();
    Sidebar::static_type();
    SidebarAvatar::static_type();
    SidebarMiniThumbnail::static_type();
    SidebarRow::static_type();
    SidebarSearch::static_type();
    SidebarSearchItemRow::static_type();
    SidebarSearchRow::static_type();
    SidebarSearchSection::static_type();
    SidebarSearchSectionRow::static_type();
    SidebarSearchSectionType::static_type();
    Snow::static_type();
    Sticker::static_type();
    Window::static_type();
}
