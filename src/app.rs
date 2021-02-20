// Shortwave - app.rs
// Copyright (C) 2021  Felix Häcker <haeckerfelix@gnome.org>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use gio::subclass::prelude::ApplicationImpl;
use gio::SettingsExt;
use glib::clone;
use glib::subclass;
use glib::subclass::prelude::*;
use glib::{Receiver, Sender};
use gtk::prelude::*;
use gtk::subclass::application::GtkApplicationImpl;
use gtk::{gdk, gio, glib};

use std::cell::RefCell;
use std::env;
use std::rc::Rc;
use std::str::FromStr;

use crate::api::SwStation;
use crate::audio::{GCastDevice, PlaybackState, Player, Song};
use crate::config;
use crate::database::Library;
use crate::settings::{settings_manager, Key};
use crate::ui::{Notification, SwApplicationWindow, View};
use crate::utils::{Order, Sorting};

#[derive(Debug, Clone)]
pub enum Action {
    ViewGoBack,
    ViewShowDiscover,
    ViewShowLibrary,
    ViewShowSearch,
    ViewShowPlayer,
    ViewEnableMiniPlayer,
    ViewDisableMiniPlayer,
    ViewRaise,
    ViewShowNotification(Rc<Notification>),
    PlaybackConnectGCastDevice(GCastDevice),
    PlaybackDisconnectGCastDevice,
    PlaybackSetStation(Box<SwStation>),
    PlaybackStart,
    PlaybackStop,
    PlaybackToggleStartStop,
    PlaybackSetVolume(f64),
    PlaybackSaveSong(Song),
    LibraryAddStations(Vec<SwStation>),
    LibraryRemoveStations(Vec<SwStation>),
    SettingsKeyChanged(Key),
}

pub struct SwApplicationPrivate {
    sender: Sender<Action>,
    receiver: RefCell<Option<Receiver<Action>>>,

    window: RefCell<Option<SwApplicationWindow>>,
    pub player: Rc<Player>,
    pub library: Library,

    settings: gio::Settings,
}

impl ObjectSubclass for SwApplicationPrivate {
    const NAME: &'static str = "SwApplication";
    type ParentType = gtk::Application;
    type Instance = subclass::simple::InstanceStruct<Self>;
    type Interfaces = ();
    type Class = subclass::simple::ClassStruct<Self>;
    type Type = super::SwApplication;

    glib::object_subclass!();

    fn new() -> Self {
        let (sender, r) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let receiver = RefCell::new(Some(r));

        let window = RefCell::new(None);
        let player = Player::new(sender.clone());
        let library = Library::new(sender.clone());

        let settings = settings_manager::get_settings();

        Self {
            sender,
            receiver,
            window,
            player,
            library,
            settings,
        }
    }
}

// Implement GLib.OBject for SwApplication
impl ObjectImpl for SwApplicationPrivate {}

// Implement Gtk.Application for SwApplication
impl GtkApplicationImpl for SwApplicationPrivate {}

// Implement Gio.Application for SwApplication
impl ApplicationImpl for SwApplicationPrivate {
    fn activate(&self, _app: &Self::Type) {
        debug!("gio::Application -> activate()");
        let app = ObjectSubclass::get_instance(self).downcast::<SwApplication>().unwrap();

        // If the window already exists,
        // present it instead creating a new one again.
        if let Some(ref window) = *self.window.borrow() {
            window.present();
            info!("Application window presented.");
            return;
        }

        // No window available -> we have to create one
        let window = app.create_window();
        window.present();
        self.window.replace(Some(window));
        info!("Created application window.");

        // Setup action channel
        let receiver = self.receiver.borrow_mut().take().unwrap();
        receiver.attach(None, move |action| app.process_action(action));

        // Setup settings signal (we get notified when a key gets changed)
        self.settings.connect_changed(clone!(@strong self.sender as sender => move |_, key_str| {
            let key: Key = Key::from_str(key_str).unwrap();
            send!(sender, Action::SettingsKeyChanged(key));
        }));

        // List all setting keys
        settings_manager::list_keys();

        // Make sure only supported themes are getting applied
        let settings = gtk::Settings::get_default().unwrap();
        settings.connect_property_gtk_theme_name_notify(|_| SwApplication::check_theme());
        settings.connect_property_gtk_icon_theme_name_notify(|_| SwApplication::check_theme());
        SwApplication::check_theme();

        // Small workaround to update every view to the correct sorting/order.
        send!(self.sender, Action::SettingsKeyChanged(Key::ViewSorting));
    }
}

// Wrap SwApplicationPrivate into a usable gtk-rs object
glib::wrapper! {
    pub struct SwApplication(ObjectSubclass<SwApplicationPrivate>)
        @extends gio::Application, gtk::Application;
}

// SwApplication implementation itself
impl SwApplication {
    pub fn run() {
        info!("{} ({}) ({})", config::NAME, config::APP_ID, config::VCS_TAG);
        info!("Version: {} ({})", config::VERSION, config::PROFILE);
        info!("Isahc version: {}", isahc::version());

        // Create new GObject and downcast it into SwApplication
        let app = glib::Object::new::<SwApplication>(&[("application-id", &Some(config::APP_ID)), ("flags", &gio::ApplicationFlags::empty())]).unwrap();

        // Start running gtk::Application
        let args: Vec<String> = env::args().collect();
        ApplicationExtManual::run(&app, &args);
    }

    fn create_window(&self) -> SwApplicationWindow {
        let self_ = SwApplicationPrivate::from_instance(self);
        let window = SwApplicationWindow::new(self_.sender.clone(), self.clone());

        // Load custom styling
        let p = gtk::CssProvider::new();
        gtk::CssProvider::load_from_resource(&p, "/de/haeckerfelix/Shortwave/gtk/style.css");
        gtk::StyleContext::add_provider_for_display(&gdk::Display::get_default().unwrap(), &p, 500);

        // Set initial view
        window.set_view(View::Library);

        // Setup help overlay
        let builder = gtk::Builder::from_resource("/de/haeckerfelix/Shortwave/gtk/shortcuts.ui");
        get_widget!(builder, gtk::ShortcutsWindow, shortcuts);
        let w = window.clone().upcast::<gtk::ApplicationWindow>();
        w.set_help_overlay(Some(&shortcuts));

        window
    }

    fn check_theme() {
        let settings = gtk::Settings::get_default().unwrap();

        let allowed_themes = vec!["Adwaita", "Adwaita:dark", "Adwaita-dark", "HighContrast", "HighContrastInverse"];
        let allowed_icon_themes = vec!["Adwaita", "HighContrast"];

        let theme_name = settings.get_property_gtk_theme_name().unwrap();
        let icon_theme_name = settings.get_property_gtk_icon_theme_name().unwrap();

        if !allowed_themes.contains(&theme_name.as_str()) {
            warn!("Detected unsupported GTK theme, fallback to Adwaita");
            settings.set_property_gtk_theme_name(Some("Adwaita"));
        }

        if !allowed_icon_themes.contains(&icon_theme_name.as_str()) {
            warn!("Detected unsupported GTK icon theme, fallback to Adwaita");
            settings.set_property_gtk_icon_theme_name(Some("Adwaita"));
        }
    }

    fn process_action(&self, action: Action) -> glib::Continue {
        let self_ = SwApplicationPrivate::from_instance(self);

        match action {
            Action::ViewGoBack => self_.window.borrow().as_ref().unwrap().go_back(),
            Action::ViewShowDiscover => self_.window.borrow().as_ref().unwrap().set_view(View::Discover),
            Action::ViewShowLibrary => self_.window.borrow().as_ref().unwrap().set_view(View::Library),
            Action::ViewShowSearch => self_.window.borrow().as_ref().unwrap().set_view(View::Search),
            Action::ViewShowPlayer => self_.window.borrow().as_ref().unwrap().set_view(View::Player),
            Action::ViewRaise => self_.window.borrow().as_ref().unwrap().present_with_time((glib::get_monotonic_time() / 1000) as u32),
            Action::ViewEnableMiniPlayer => self_.window.borrow().as_ref().unwrap().enable_mini_player(true),
            Action::ViewDisableMiniPlayer => self_.window.borrow().as_ref().unwrap().enable_mini_player(false),
            Action::ViewShowNotification(notification) => self_.window.borrow().as_ref().unwrap().show_notification(notification),
            Action::PlaybackConnectGCastDevice(device) => self_.player.connect_to_gcast_device(device),
            Action::PlaybackDisconnectGCastDevice => self_.player.disconnect_from_gcast_device(),
            Action::PlaybackSetStation(station) => {
                self_.player.set_station(*station);
                self_.window.borrow().as_ref().unwrap().show_player_widget();
            }
            Action::PlaybackStart => self_.player.set_playback(PlaybackState::Playing),
            Action::PlaybackStop => self_.player.set_playback(PlaybackState::Stopped),
            Action::PlaybackToggleStartStop => self_.player.toggle_playback(),
            Action::PlaybackSetVolume(volume) => self_.player.set_volume(volume),
            Action::PlaybackSaveSong(song) => self_.player.save_song(song),
            Action::LibraryAddStations(stations) => self_.library.add_stations(stations),
            Action::LibraryRemoveStations(stations) => self_.library.remove_stations(stations),
            Action::SettingsKeyChanged(key) => {
                debug!("Settings key changed: {:?}", &key);
                match key {
                    Key::ViewSorting | Key::ViewOrder => {
                        let sorting: Sorting = Sorting::from_str(&settings_manager::get_string(Key::ViewSorting)).unwrap();
                        let order: Order = Order::from_str(&settings_manager::get_string(Key::ViewOrder)).unwrap();
                        self_.library.set_sorting(sorting, order);
                    }
                    _ => (),
                }
            }
        }
        glib::Continue(true)
    }
}
