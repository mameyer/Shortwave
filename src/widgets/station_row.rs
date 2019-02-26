use glib::Sender;
use gtk::prelude::*;
use rustio::Station;

use crate::app::Action;
use crate::widgets::station_infobox::StationInfobox;

#[derive(Clone)]
pub enum ContentType {
    Library,
    Other,
}

pub struct StationRow {
    pub widget: gtk::ListBoxRow,
    station: Station,

    builder: gtk::Builder,
    sender: Sender<Action>,
}

impl StationRow {
    pub fn new(sender: Sender<Action>, station: Station, content_type: ContentType) -> Self {
        let builder = gtk::Builder::new_from_resource("/de/haeckerfelix/Shortwave/gtk/station_row.ui");
        let row: gtk::ListBoxRow = builder.get_object("station_row").unwrap();

        // Set row information
        let station_label: gtk::Label = builder.get_object("station_label").unwrap();
        let location_label: gtk::Label = builder.get_object("location_label").unwrap();
        let votes_label: gtk::Label = builder.get_object("votes_label").unwrap();
        station_label.set_text(&station.name);
        location_label.set_text(&format!("{} {}", station.country, station.state));
        votes_label.set_text(&format!("{} Votes", station.votes));

        // Station Info Box
        let info = StationInfobox::new();
        info.set_station(&station);
        let info_box: gtk::Box = builder.get_object("info_box").unwrap();
        info_box.add(&info.widget);

        // Wether to show 'add' or 'remove' button
        let library_action_stack: gtk::Stack = builder.get_object("library_action_stack").unwrap();
        match content_type {
            ContentType::Library => library_action_stack.set_visible_child_name("library-remove"),
            ContentType::Other => library_action_stack.set_visible_child_name("library-add"),
        }

        let stationrow = Self {
            widget: row,
            station,
            builder,
            sender,
        };

        stationrow.setup_signals();
        stationrow
    }

    fn setup_signals(&self) {
        // play_button
        let play_button: gtk::Button = self.builder.get_object("play_button").unwrap();
        let sender = self.sender.clone();
        let station = self.station.clone();
        play_button.connect_clicked(move |_| {
            sender.send(Action::PlaybackSetStation(station.clone())).unwrap();
        });

        // remove_button
        let remove_button: gtk::Button = self.builder.get_object("remove_button").unwrap();
        let sender = self.sender.clone();
        let station = self.station.clone();
        remove_button.connect_clicked(move |btn| {
            sender.send(Action::LibraryRemoveStations(vec![station.clone()])).unwrap();
            btn.set_sensitive(false);
        });

        // add_button
        let add_button: gtk::Button = self.builder.get_object("add_button").unwrap();
        let sender = self.sender.clone();
        let station = self.station.clone();
        add_button.connect_clicked(move |btn| {
            sender.send(Action::LibraryAddStations(vec![station.clone()])).unwrap();
            btn.set_sensitive(false);
        });

        // eventbox
        let eventbox: gtk::EventBox = self.builder.get_object("eventbox").unwrap();
        let check_button: gtk::CheckButton = self.builder.get_object("check_button").unwrap();
        let revealer: gtk::Revealer = self.builder.get_object("revealer").unwrap();
        eventbox.connect_button_press_event(move |_, button| {
            // 3 -> Right mouse button
            if button.get_button() == 3 {
                // TODO: enable selection mode
                check_button.set_active(true);
            } else {
                // TODO: handle selection mode - check_button.set_active(!check_button.get_active());
                revealer.set_reveal_child(!revealer.get_reveal_child());
            }
            gtk::Inhibit(false)
        });
    }
}
