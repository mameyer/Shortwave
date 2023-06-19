// Shortwave - map_page.rs
// Copyright (C) 2023  Felix Häcker <haeckerfelix@gnome.org>
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

use adw::subclass::prelude::*;
use glib::{clone, subclass};
use gtk::prelude::*;
use gtk::{glib, CompositeTemplate};
use once_cell::unsync::OnceCell;
use shumate::prelude::*;
use shumate::{Map, MapLayer, Marker, MarkerLayer};

use crate::api::{StationRequest, SwClient, SwStation};
use crate::app::SwApplication;
use crate::ui::SwStationDialog;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/map_page.ui")]
    pub struct SwMapPage {
        #[template_child]
        pub map: TemplateChild<Map>,

        map_layer: OnceCell<MapLayer>,
        marker_layer: OnceCell<MarkerLayer>,
        pub client: SwClient,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwMapPage {
        const NAME: &'static str = "SwMapPage";
        type ParentType = adw::Bin;
        type Type = super::SwMapPage;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwMapPage {
        fn constructed(&self) {
            let viewport = self.map.viewport().unwrap();

            let marker_layer = MarkerLayer::new(&viewport);
            self.map.add_layer(&marker_layer);
            self.marker_layer.set(marker_layer).unwrap();

            self.client.model().connect_items_changed(
                clone!(@weak self as this => move|model, pos, removed, added|{
                    for i in 0..added {
                        let station: SwStation = model.item(pos + i).unwrap().downcast().unwrap();
                        this.add_station_marker(&station);
                    }

                    for i in 0..removed {
                        let _station: SwStation = model.item(pos + i).unwrap().downcast().unwrap();
                        // TODO:: this.remove_station_marker(&station);
                    }
                }),
            );
        }
    }

    impl WidgetImpl for SwMapPage {}

    impl BinImpl for SwMapPage {}

    impl SwMapPage {
        pub fn ensure_map_layer(&self) {
            if self.map_layer.get().is_none() {
                let registry = shumate::MapSourceRegistry::with_defaults();
                let source = registry.by_id(shumate::MAP_SOURCE_OSM_MAPNIK).unwrap();
                self.map.set_map_source(&source);

                let viewport = self.map.viewport().unwrap();
                viewport.set_reference_map_source(Some(&source));
                viewport.set_zoom_level(3.0);

                let layer = MapLayer::new(&source, &viewport);
                self.map.insert_layer_above(&layer, None::<&MapLayer>);
            }
        }

        fn add_station_marker(&self, station: &SwStation) {
            let long: f64 = station.metadata().geo_long.unwrap_or(0.0).into();
            let lat: f64 = station.metadata().geo_lat.unwrap_or(0.0).into();

            if long != 0.0 || lat != 0.0 {
                let marker = Marker::new();

                let marker_button = gtk::Button::new();
                marker_button.add_css_class("flat");
                marker_button.add_css_class("map-pin");
                marker_button.set_icon_name("mark-location-symbolic");
                marker_button.connect_clicked(clone!(@weak station => move |_|{
                    let app = SwApplication::default();
                    let sender = app.sender();

                    let station_dialog = SwStationDialog::new(sender, station);
                    station_dialog.show();
                }));

                marker.set_child(Some(&marker_button));
                marker.set_location(lat, long);

                self.marker_layer.get().unwrap().add_marker(&marker);
            }
        }
    }
}

glib::wrapper! {
    pub struct SwMapPage(ObjectSubclass<imp::SwMapPage>)
        @extends gtk::Widget, adw::Bin;
}

impl SwMapPage {
    pub fn update_data(&self) {
        let imp = self.imp();

        imp.ensure_map_layer();

        let request = StationRequest {
            has_geo_info: Some(true),
            limit: Some(50),
            order: Some("votes".to_string()),
            ..StationRequest::default()
        };

        // TODO: error handling
        warn!("Send request");
        imp.client.send_station_request(request);
    }
}
