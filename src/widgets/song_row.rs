use chrono::NaiveTime;
use glib::Sender;
use gtk::prelude::*;
use libhandy::{ActionRow, ActionRowExt};

use std::path::PathBuf;

use crate::app::Action;
use crate::song::Song;

pub struct SongRow {
    pub widget: ActionRow,
    song: Song,
    save_button: gtk::Button,
}

impl SongRow {
    pub fn new(_sender: Sender<Action>, song: Song) -> Self {
        let widget = ActionRow::new();
        widget.set_title(&song.title);
        widget.set_subtitle(&Self::format_duration(song.duration.as_secs()));
        widget.set_icon_name("");

        let save_button = gtk::Button::new();
        save_button.set_relief(gtk::ReliefStyle::None);
        save_button.set_valign(gtk::Align::Center);
        let save_image = gtk::Image::new_from_icon_name("document-save-symbolic", gtk::IconSize::__Unknown(4));
        save_button.add(&save_image);
        widget.add_action(&save_button);
        widget.show_all();

        let row = Self { widget, song, save_button };

        row.setup_signals();
        row
    }

    fn setup_signals(&self) {
        let song = self.song.clone();
        let widget = self.widget.clone();
        let save_button = self.save_button.clone();
        self.save_button.connect_clicked(move |_| {
            let mut path = PathBuf::from(glib::get_user_special_dir(glib::UserDirectory::Music).unwrap());
            path.push(&song.title);
            match song.save_as(path) {
                Ok(()) => widget.set_subtitle("Saved"),
                Err(_) => widget.set_subtitle("Could not save"),
            };
            save_button.set_visible(false);
        });
    }

    // stolen from gnome-podcasts
    // https://gitlab.gnome.org/haecker-felix/podcasts/blob/2f8a6a91f87d7fa335a954bbaf2f70694f32f6dd/podcasts-gtk/src/widgets/player.rs#L168
    fn format_duration(seconds: u64) -> String {
        let time = NaiveTime::from_num_seconds_from_midnight(seconds as u32, 0);

        if seconds >= 3600 {
            time.format("%T").to_string()
        } else {
            time.format("%M∶%S").to_string()
        }
    }
}