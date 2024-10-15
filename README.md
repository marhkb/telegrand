<h1 align="center">
  <img src="data/icons/app.drey.PaperPlane.svg" alt="Paper Plane" width="192" height="192"/>
  <br>
  Paper Plane
</h1>

<p align="center"><strong>Chat over Telegram on a modern and elegant client</strong></p>

<p align="center">
  <a href="https://hosted.weblate.org/engage/paper-plane/">
    <img src="https://hosted.weblate.org/widgets/paper-plane/-/main/svg-badge.svg" alt="Translation status" />
  </a>
  <a href="https://github.com/paper-plane-developers/paper-plane/actions/workflows/ci.yml">
    <img src="https://github.com/paper-plane-developers/paper-plane/actions/workflows/ci.yml/badge.svg" alt="CI status"/>
  </a>
  <a href="https://repology.org/project/paper-plane/versions">
    <img src="https://repology.org/badge/tiny-repos/paper-plane.svg" alt="Packaging status">
  </a>
  <a href="https://t.me/paperplanechat">
    <img src="https://img.shields.io/static/v1?label=Chat&message=@paperplanechat&color=blue&logo=telegram" alt="Telegram group">
  </a>
</p>

<br>

<p align="center">
  <img width=600 src="data/resources/screenshots/screenshot1.png" alt="Screenshot"/>
</p>

Paper Plane is an alternative Telegram client.
It uses libadwaita for its user interface and strives to meet the design principles of the GNOME desktop.

Paper Plane is still under development and not yet feature-complete.
However, the following things are already working:

- The use of multiple accounts at the same time.
- Viewing text messages, images, stickers and files.
- Sending text messages and images.
- Replying to messages.
- Searching for groups and persons.

## Installation Instructions

<a href="https://repology.org/project/paper-plane/versions">
    <img src="https://repology.org/badge/vertical-allrepos/paper-plane.svg" alt="Packaging status" align="right">
</a>

Paper Plane is an in-development project and it isn't considered stable software yet.

Also, the included API credentials in the Flathub release are still very new and untested. In some cases, your account may end up banned by Telegram (check the `Telegram API Credentials` section below).

You can avoid that by using a custom built version of Paper Plane with provided API credentials via meson options, like [this AUR package](https://aur.archlinux.org/packages/paper-plane-git) which you may prefer using if you use Arch Linux. These API credentials
are much older and thus the risk of getting banned is reduced.

### Flathub Beta

But, if you still feel brave enough, you can install the latest release from `Flathub Beta`. To do this, you need to add the the Flathub Beta remote first
```shell
flatpak remote-add --if-not-exists flathub-beta https://flathub.org/beta-repo/flathub-beta.flatpakrepo
```
Then you can install the application by issuing
```shell
flatpak install flathub-beta app.drey.PaperPlane
```
Paper Plane can be kept up to date by issuing flatpak's update command like
```shell
flatpak update
```

### CI Build (Not Recommended)

You can also grab the latest CI build with test API credentials from [here](https://nightly.link/paper-plane-developers/paper-plane/workflows/ci/main).
Then you need to unzip the archive's content and install the application with the command `flatpak install paper-plane.flatpak`. Keep in mind that these test credentials are even more riskier than those from the Flathub release. Also, you need to manually keep it updated.

## Telegram API Credentials

Telegram requires custom clients to set some credentials for using their API. Paper Plane doesn't provide official API credentials, so the packagers are expected to set their own credentials for distributing the app, obtainable at https://my.telegram.org/. However, Paper Plane includes the Telegram's test credentials by default, which are very limited, but usable (especially for development). However, it's known that Telegram sometimes decides to ban accounts that use such credentials (especially newer accounts). For that reason, it's suggested to use your own API credentials, which can be set by using meson options (see the `Build Instructions` section below).

## Build Instructions

### Gnome Builder

Using Gnome Builder is the easiest way to get the app built without even using the terminal: just clone the repository and press the big "Run" button at the top and it will automatically build all the required dependencies together with the app.

### Meson

#### Prerequisites

The following packages are required to build Paper Plane:

- meson
- cargo
- GTK >= 4.10 (with the patch included in the build-aux directory)
- libadwaita >= 1.4
- libshumate >= 1.0.0
- [TDLib 1.8.19](https://github.com/tdlib/td/commit/2589c3fd46925f5d57e4ec79233cd1bd0f5d0c09)
- [Telegram API Credentials](https://my.telegram.org/) (optional, but recommended)

Additionally, Paper Plane requires the following GStreamer plugins installed in your system to correctly show all media files:

- gstreamer-libav
- gstreamer-plugins-good

#### Instructions

```shell
meson . _build -Dtg_api_id=ID -Dtg_api_hash=HASH
ninja -C _build
sudo ninja -C _build install
```

## Contribution

Any type of participation is encouraged. If you want to translate, you can refer to [our weblate project](https://hosted.weblate.org/engage/paper-plane). But also design and art contributions are welcome. For this [our design repository](https://github.com/paper-plane-developers/paper-plane-designs) is the first place to go.

If you want to contribute code, please keep your commits in the style of [conventional commits](https://www.conventionalcommits.org/en/v1.0.0). The only difference we make is that we capitalize the actual description after the colon ":" at the beginning of the sentence.


## Acknowledgment

The general code architecture was heavily inspired by [Fractal](https://gitlab.gnome.org/GNOME/fractal).

Also, some logic is inspired by [Telegram X](https://github.com/TGX-Android/Telegram-X), which helps to understand how to use some TDLib features correctly and to their fullest potential.
