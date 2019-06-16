use glib::Sender;
use gtk::prelude::*;

use crate::api::Station;
use crate::app::Action;
use crate::widgets::station_dialog::StationDialog;

pub struct StationRow {
    pub widget: gtk::FlowBoxChild,
    station: Station,
    app: gtk::Application,

    builder: gtk::Builder,
    sender: Sender<Action>,
}

impl StationRow {
    pub fn new(sender: Sender<Action>, station: Station) -> Self {
        let builder = gtk::Builder::new_from_resource("/de/haeckerfelix/Shortwave/gtk/station_row.ui");
        let row: gtk::FlowBoxChild = builder.get_object("station_row").unwrap();
        let app = builder.get_application().unwrap();

        // Set row information
        let station_label: gtk::Label = builder.get_object("station_label").unwrap();
        let subtitle_label: gtk::Label = builder.get_object("subtitle_label").unwrap();
        station_label.set_text(&station.name);
        subtitle_label.set_text(&format!("{} {} · {} Votes", station.country, station.state, station.votes));

        let stationrow = Self {
            widget: row,
            station,
            app,
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

        // eventbox
        let station = self.station.clone();
        let app = self.app.clone();
        let eventbox: gtk::EventBox = self.builder.get_object("eventbox").unwrap();
        let check_button: gtk::CheckButton = self.builder.get_object("check_button").unwrap();
        let sender = self.sender.clone();
        eventbox.connect_button_press_event(move |_, button| {
            // 3 -> Right mouse button
            if button.get_button() == 3 {
                // TODO: enable selection mode
                check_button.set_active(true);
            } else {
                let window = app.get_active_window().unwrap();
                let station_dialog = StationDialog::new(sender.clone(), station.clone(), &window);
                station_dialog.show();
            }
            gtk::Inhibit(false)
        });
    }
}
