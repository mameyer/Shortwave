use gio::prelude::*;
use glib::Sender;
use mpris_player::{Metadata, MprisPlayer, OrgMprisMediaPlayer2Player, PlaybackStatus};

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use crate::api::FaviconDownloader;
use crate::api::Station;
use crate::app::Action;
use crate::audio::Controller;
use crate::audio::PlaybackState;
use crate::config;

pub struct MprisController {
    sender: Sender<Action>,
    mpris: Arc<MprisPlayer>,

    song_title: Cell<Option<String>>,
    station: Cell<Option<Station>>,
    volume: Rc<RefCell<f64>>,
}

impl MprisController {
    pub fn new(sender: Sender<Action>) -> Self {
        let mpris = MprisPlayer::new(config::APP_ID.to_string(), config::NAME.to_string(), config::APP_ID.to_string());
        mpris.set_can_raise(true);
        mpris.set_can_play(false);
        mpris.set_can_seek(false);
        mpris.set_can_set_fullscreen(false);
        mpris.set_can_pause(true);

        let volume = Rc::new(RefCell::new(0.0));

        let controller = Self {
            sender,
            mpris,
            song_title: Cell::new(None),
            station: Cell::new(None),
            volume,
        };

        controller.setup_signals();
        controller
    }

    fn update_metadata(&self) {
        let mut metadata = Metadata::new();

        let station = self.station.take();
        let song_title = self.song_title.take();

        station.clone().map(|station| {
            station.favicon.map(|favicon| {
                FaviconDownloader::get_file(&favicon).map(|file| {
                    let path = format!("file://{}", file.get_path().unwrap().to_str().unwrap().to_owned());
                    metadata.art_url = Some(path);
                })
            });
            metadata.artist = Some(vec![station.name]);
        });
        song_title.clone().map(|song_title| {
            metadata.title = Some(song_title);
        });

        self.station.set(station);
        self.song_title.set(song_title);

        self.mpris.set_metadata(metadata);
    }

    fn setup_signals(&self) {
        // mpris raise
        let sender = self.sender.clone();
        self.mpris.connect_raise(move || {
            send!(sender, Action::ViewRaise);
        });

        // mpris play / pause
        let sender = self.sender.clone();
        let mpris = self.mpris.clone();
        self.mpris.connect_play_pause(move || {
            match mpris.get_playback_status().unwrap().as_ref() {
                "Paused" => send!(sender, Action::PlaybackStart),
                "Stopped" => send!(sender, Action::PlaybackStart),
                _ => send!(sender, Action::PlaybackStop),
            };
        });

        // mpris play
        let sender = self.sender.clone();
        self.mpris.connect_play(move || {
            send!(sender, Action::PlaybackStart);
        });

        // mpris stop
        let sender = self.sender.clone();
        self.mpris.connect_stop(move || {
            send!(sender, Action::PlaybackStop);
        });

        // mpris pause
        let sender = self.sender.clone();
        self.mpris.connect_pause(move || {
            send!(sender, Action::PlaybackStop);
        });

        // mpris volume
        let sender = self.sender.clone();
        let old_volume = self.volume.clone();
        self.mpris.connect_volume(move |new_volume| {
            if *old_volume.borrow() != new_volume {
                send!(sender, Action::PlaybackSetVolume(new_volume.clone()));
                *old_volume.borrow_mut() = new_volume;
            }
        });
    }
}

impl Controller for MprisController {
    fn set_station(&self, station: Station) {
        self.station.set(Some(station));
        self.update_metadata();
    }

    fn set_playback_state(&self, playback_state: &PlaybackState) {
        self.mpris.set_can_play(true);

        match playback_state {
            PlaybackState::Playing => self.mpris.set_playback_status(PlaybackStatus::Playing),
            _ => self.mpris.set_playback_status(PlaybackStatus::Stopped),
        };
    }

    fn set_volume(&self, volume: f64) {
        *self.volume.borrow_mut() = volume;
        self.mpris.set_volume(volume.clone()).unwrap();
    }

    fn set_song_title(&self, title: &str) {
        self.song_title.set(Some(title.to_string()));
        self.update_metadata();
    }
}
