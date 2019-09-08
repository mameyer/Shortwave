use glib::Sender;
use gtk::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

use crate::api::Station;
use crate::app::Action;
use crate::audio::Controller;
use crate::audio::PlaybackState;
use crate::ui::StationDialog;

pub struct SidebarController {
    pub widget: gtk::Box,
    sender: Sender<Action>,
    station: Rc<RefCell<Option<Station>>>,
    app: gtk::Application,

    title_label: gtk::Label,
    subtitle_label: gtk::Label,
    subtitle_revealer: gtk::Revealer,
    action_revealer: gtk::Revealer,
    playback_button_stack: gtk::Stack,
    start_playback_button: gtk::Button,
    stop_playback_button: gtk::Button,
    info_button: gtk::Button,
    error_label: gtk::Label,
}

impl SidebarController {
    pub fn new(sender: Sender<Action>) -> Self {
        let builder = gtk::Builder::new_from_resource("/de/haeckerfelix/Shortwave/gtk/sidebar_controller.ui");

        let station = Rc::new(RefCell::new(None));
        let app = builder.get_application().unwrap();

        let widget: gtk::Box = builder.get_object("sidebar_controller").unwrap();
        let title_label: gtk::Label = builder.get_object("title_label").unwrap();
        let subtitle_label: gtk::Label = builder.get_object("subtitle_label").unwrap();
        let subtitle_revealer: gtk::Revealer = builder.get_object("subtitle_revealer").unwrap();
        let action_revealer: gtk::Revealer = builder.get_object("action_revealer").unwrap();
        let playback_button_stack: gtk::Stack = builder.get_object("playback_button_stack").unwrap();
        let start_playback_button: gtk::Button = builder.get_object("start_playback_button").unwrap();
        let stop_playback_button: gtk::Button = builder.get_object("stop_playback_button").unwrap();
        let info_button: gtk::Button = builder.get_object("info_button").unwrap();
        let error_label: gtk::Label = builder.get_object("error_label").unwrap();

        let controller = Self {
            widget,
            sender,
            station,
            app,
            title_label,
            subtitle_label,
            action_revealer,
            subtitle_revealer,
            playback_button_stack,
            start_playback_button,
            stop_playback_button,
            info_button,
            error_label,
        };

        controller.setup_signals();
        controller
    }

    fn setup_signals(&self) {
        // start_playback_button
        let sender = self.sender.clone();
        self.start_playback_button.connect_clicked(move |_| {
            sender.send(Action::PlaybackStart).unwrap();
        });

        // stop_playback_button
        let sender = self.sender.clone();
        self.stop_playback_button.connect_clicked(move |_| {
            sender.send(Action::PlaybackStop).unwrap();
        });

        // info_button
        let station = self.station.clone();
        let app = self.app.clone();
        let sender = self.sender.clone();
        self.info_button.connect_clicked(move |_| {
            let window = app.get_active_window().unwrap();
            let s = station.borrow().clone().unwrap();

            let station_dialog = StationDialog::new(sender.clone(), s, &window);
            station_dialog.show();
        });
    }
}

impl Controller for SidebarController {
    fn set_station(&self, station: Station) {
        self.action_revealer.set_reveal_child(true);
        self.title_label.set_text(&station.name);
        self.title_label.set_tooltip_text(Some(station.name.as_str()));
        *self.station.borrow_mut() = Some(station);

        // reset everything else
        self.error_label.set_text(" ");
        self.subtitle_revealer.set_reveal_child(false);
    }

    fn set_playback_state(&self, playback_state: &PlaybackState) {
        match playback_state {
            PlaybackState::Playing => self.playback_button_stack.set_visible_child_name("stop_playback"),
            PlaybackState::Stopped => self.playback_button_stack.set_visible_child_name("start_playback"),
            PlaybackState::Loading => self.playback_button_stack.set_visible_child_name("loading"),
            PlaybackState::Failure(msg) => {
                self.playback_button_stack.set_visible_child_name("error");
                let mut text = self.error_label.get_text().unwrap().to_string();
                text = text + " " + msg;
                self.error_label.set_text(&text);
            }
        };
    }

    fn set_song_title(&self, title: &str) {
        if title != "" {
            self.subtitle_label.set_text(title);
            self.subtitle_label.set_tooltip_text(Some(title));
            self.subtitle_revealer.set_reveal_child(true);
        } else {
            self.subtitle_label.set_text("");
            self.subtitle_label.set_tooltip_text(None);
            self.subtitle_revealer.set_reveal_child(false);
        }
    }
}
