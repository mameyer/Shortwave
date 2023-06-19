// Shortwave - app.rs
// Copyright (C) 2021-2023  Felix Häcker <haeckerfelix@gnome.org>
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

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use adw::subclass::prelude::*;
use gio::subclass::prelude::ApplicationImpl;
use glib::{clone, ObjectExt, ParamSpec, Properties, Receiver, Sender};
use gtk::glib::WeakRef;
use gtk::prelude::*;
use gtk::{gio, glib};
use once_cell::sync::OnceCell;

use crate::api::{SwClient, SwStation};
use crate::audio::{GCastDevice, PlaybackState, Player, Song};
use crate::config;
use crate::database::SwLibrary;
use crate::model::SwSorting;
use crate::settings::{settings_manager, Key, SettingsWindow};
use crate::ui::{about_window, SwApplicationWindow, SwView};

#[derive(Debug, Clone)]
pub enum Action {
    // Audio Playback
    PlaybackConnectGCastDevice(GCastDevice),
    PlaybackDisconnectGCastDevice,
    PlaybackSetStation(Box<SwStation>),
    PlaybackSet(bool),
    PlaybackToggle,
    PlaybackSetVolume(f64),
    PlaybackSaveSong(Song),

    SettingsKeyChanged(Key),
}

mod imp {
    use super::*;

    #[derive(Properties)]
    #[properties(wrapper_type = super::SwApplication)]
    pub struct SwApplication {
        #[property(get)]
        pub library: SwLibrary,
        #[property(get)]
        pub rb_server: RefCell<Option<String>>,

        pub sender: Sender<Action>,
        pub receiver: RefCell<Option<Receiver<Action>>>,

        pub window: OnceCell<WeakRef<SwApplicationWindow>>,
        pub player: Rc<Player>,

        pub settings: gio::Settings,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwApplication {
        const NAME: &'static str = "SwApplication";
        type ParentType = adw::Application;
        type Type = super::SwApplication;

        fn new() -> Self {
            let (sender, r) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
            let receiver = RefCell::new(Some(r));

            let library = SwLibrary::new(sender.clone());
            let rb_server = RefCell::default();

            let window = OnceCell::new();
            let player = Player::new(sender.clone());

            let settings = settings_manager::settings();

            Self {
                library,
                rb_server,
                sender,
                receiver,
                window,
                player,
                settings,
            }
        }
    }

    // Implement GLib.Object for SwApplication
    impl ObjectImpl for SwApplication {
        fn properties() -> &'static [ParamSpec] {
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }
    }

    // Implement Gio.Application for SwApplication
    impl ApplicationImpl for SwApplication {
        fn activate(&self) {
            debug!("gio::Application -> activate()");
            let app = self.obj();

            // If the window already exists,
            // present it instead creating a new one again.
            if let Some(weak_window) = self.window.get() {
                weak_window.upgrade().unwrap().present();
                info!("Application window presented.");
                return;
            }

            // No window available -> we have to create one
            let window = app.create_window();
            let _ = self.window.set(window.downgrade());
            info!("Created application window.");

            // Setup app level GActions
            app.setup_gactions();

            // Setup action channel
            let receiver = self.receiver.borrow_mut().take().unwrap();
            receiver.attach(
                None,
                clone!(@strong app => move |action| app.process_action(action)),
            );

            // Connect with radiobrowser server and update library data
            app.lookup_rb_server();

            // Setup settings signal (we get notified when a key gets changed)
            self.settings.connect_changed(
                None,
                clone!(@strong self.sender as sender => move |_, key_str| {
                    let key: Key = Key::from_str(key_str).unwrap();
                    send!(sender, Action::SettingsKeyChanged(key));
                }),
            );

            // Small workaround to update every view to the correct sorting/order.
            send!(self.sender, Action::SettingsKeyChanged(Key::ViewSorting));
        }
    }

    // Implement Gtk.Application for SwApplication
    impl GtkApplicationImpl for SwApplication {}

    // Implement Adw.Application for SwApplication
    impl AdwApplicationImpl for SwApplication {}
}

// Wrap SwApplication into a usable gtk-rs object
glib::wrapper! {
    pub struct SwApplication(ObjectSubclass<imp::SwApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

// SwApplication implementation itself
impl SwApplication {
    pub fn run() -> glib::ExitCode {
        debug!(
            "{} ({}) ({}) - Version {} ({})",
            config::NAME,
            config::APP_ID,
            config::VCS_TAG,
            config::VERSION,
            config::PROFILE
        );
        info!("Isahc version: {}", isahc::version());

        // Create new GObject and downcast it into SwApplication
        let app = glib::Object::builder::<SwApplication>()
            .property("application-id", Some(config::APP_ID))
            .property("flags", gio::ApplicationFlags::empty())
            .property("resource-base-path", Some(config::PATH_ID))
            .build();

        // Start running gtk::Application
        app.run()
    }

    fn create_window(&self) -> SwApplicationWindow {
        let imp = self.imp();
        let window = SwApplicationWindow::new(imp.sender.clone(), self.clone(), imp.player.clone());

        // Set initial view
        window.set_view(SwView::Library);

        window.present();
        window
    }

    fn setup_gactions(&self) {
        let window = SwApplicationWindow::default();

        self.add_action_entries([
            // app.show-preferences
            gio::ActionEntry::builder("show-preferences")
                .activate(clone!(@weak window => move |_, _, _| {
                    let settings_window = SettingsWindow::new(&window.upcast());
                    settings_window.show();
                }))
                .build(),
            // app.quit
            gio::ActionEntry::builder("quit")
                .activate(clone!(@weak window => move |_, _, _| {
                    window.close();
                }))
                .build(),
            // app.about
            gio::ActionEntry::builder("about")
                .activate(clone!(@weak window => move |_, _, _| {
                    about_window::show(&window);
                }))
                .build(),
        ]);
        self.set_accels_for_action("app.show-preferences", &["<primary>comma"]);
        self.set_accels_for_action("app.quit", &["<primary>q"]);
        self.set_accels_for_action("window.close", &["<primary>w"]);
    }

    fn process_action(&self, action: Action) -> glib::Continue {
        let imp = self.imp();
        if self.active_window().is_none() {
            return glib::Continue(true);
        }

        let window = SwApplicationWindow::default();

        match action {
            Action::PlaybackConnectGCastDevice(device) => {
                imp.player.connect_to_gcast_device(device)
            }
            Action::PlaybackDisconnectGCastDevice => imp.player.disconnect_from_gcast_device(),
            Action::PlaybackSetStation(station) => {
                imp.player.set_station(*station);
                window.show_player_widget();
            }
            Action::PlaybackSet(true) => imp.player.set_playback(PlaybackState::Playing),
            Action::PlaybackSet(false) => imp.player.set_playback(PlaybackState::Stopped),
            Action::PlaybackToggle => imp.player.toggle_playback(),
            Action::PlaybackSetVolume(volume) => imp.player.set_volume(volume),
            Action::PlaybackSaveSong(song) => imp.player.save_song(song),
            Action::SettingsKeyChanged(key) => self.apply_settings_changes(key),
        }
        glib::Continue(true)
    }

    fn apply_settings_changes(&self, key: Key) {
        debug!("Settings key changed: {:?}", &key);
        match key {
            Key::ViewSorting | Key::ViewOrder => {
                let value = settings_manager::string(Key::ViewSorting);
                let sorting = SwSorting::from_str(&value).unwrap();
                let order = settings_manager::string(Key::ViewOrder);
                let descending = order == "Descending";

                self.imp()
                    .window
                    .get()
                    .unwrap()
                    .upgrade()
                    .unwrap()
                    .set_sorting(sorting, descending);
            }
            _ => (),
        }
    }

    pub fn lookup_rb_server(&self) {
        let fut = clone!(@weak self as this => async move {
            let imp = this.imp();

            // Try to find a working radiobrowser server
            let rb_server = SwClient::lookup_rb_server().await;
            if let Some(rb_server) = &rb_server {
                info!("Using radio-browser.info REST api: {rb_server}");
            }else{
                warn!("Unable to find radio-browser.info server.");
            }

            *imp.rb_server.borrow_mut() = rb_server.clone();
            this.notify("rb-server");

            // Refresh library data either way, it'll fallback to
            // local cached data if there's no radiobrowser server available.
            imp.library.update_data();
        });
        spawn!(fut);
    }

    // TODO: Kill this, just a temporary workaround
    pub fn sender(&self) -> Sender<Action> {
        self.imp().sender.clone()
    }
}

impl Default for SwApplication {
    fn default() -> Self {
        gio::Application::default()
            .expect("Could not get default GApplication")
            .downcast()
            .unwrap()
    }
}
