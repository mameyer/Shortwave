mod gtk_controller;
mod mpris_controller;

pub use gtk_controller::GtkController;
pub use mpris_controller::MprisController;

use crate::audio::PlaybackState;
use rustio::Station;

pub trait Controller {
    fn set_station(&self, station: Station);
    fn set_playback_state(&self, playback_state: &PlaybackState);
    fn set_song_title(&self, title: &str);
}
