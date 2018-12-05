extern crate gio;
extern crate gtk;
use gtk::prelude::*;

use rustio::Station;
use std::sync::mpsc::Sender;

use app::Action;
use widgets::station_row::{StationRow, ContentType};
use station_model::{Sorting, Order, StationModel};

pub struct StationListBox {
    pub widget: gtk::Box,
    listbox: gtk::ListBox,
    station_model: StationModel,
    content_type: ContentType,

    sender: Sender<Action>,
}

impl StationListBox {
    // TODO: remove title stuff from ui file
    pub fn new(sender: Sender<Action>, content_type: ContentType) -> Self {
        let builder = gtk::Builder::new_from_resource("/de/haeckerfelix/Gradio/gtk/station_listbox.ui");
        let widget: gtk::Box = builder.get_object("station_listbox").unwrap();
        let listbox: gtk::ListBox = builder.get_object("listbox").unwrap();
        let station_model = StationModel::new();

        Self { widget, listbox, station_model, content_type, sender }
    }

    pub fn add_stations(&mut self, stations: Vec<Station>){
        for station in stations{
            match self.station_model.add_station(station.clone()){
                Some(index) => {
                    let row = StationRow::new(self.sender.clone(), station, self.content_type.clone());
                    self.listbox.insert(&row.widget, index as i32);
                },
                None => (),
            }
        }
    }

    pub fn remove_stations(&mut self, stations: Vec<Station>){
        for station in stations{
            match self.station_model.remove_station(station){
                Some(index) => {
                    let row = self.listbox.get_row_at_index(index as i32).unwrap();
                    self.listbox.remove(&row);
                }
                None => (),
            }
        }
    }

    pub fn get_stations(&self) -> Vec<Station>{
        self.station_model.export_vec()
    }

    pub fn set_sorting(&mut self, sorting: Sorting, order: Order){
        self.station_model.set_sorting(sorting, order);
        self.station_model.sort();
        self.refresh();
    }

    fn refresh(&self){
        self.clear();
        for (_, station) in self.station_model.clone() {
            let row = StationRow::new(self.sender.clone(), station, self.content_type.clone());
            self.listbox.add(&row.widget);
        }
    }

    fn clear(&self) {
        for widget in self.listbox.get_children() {
            widget.destroy();
        }
    }
}


