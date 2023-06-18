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
use glib::subclass;
use gtk::{glib, CompositeTemplate};
use shumate::{Map, MapLayer, MarkerLayer};

use crate::api::{StationRequest, SwClient};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/map_page.ui")]
    pub struct SwMapPage {
        #[template_child]
        pub map: TemplateChild<Map>,

        client: SwClient,
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
            let registry = shumate::MapSourceRegistry::with_defaults();
            let source = registry.by_id(shumate::MAP_SOURCE_OSM_MAPNIK).unwrap();
            self.map.set_map_source(&source);

            let viewport = self.map.viewport().unwrap();
            viewport.set_reference_map_source(Some(&source));
            viewport.set_zoom_level(6.0);

            let layer = MapLayer::new(&source, &viewport);
            self.map.add_layer(&layer);

            let marker_layer = MarkerLayer::new(&viewport);
            // marker_layer.add_marker(&self.marker);
            self.map.add_layer(&marker_layer);
        }
    }

    impl WidgetImpl for SwMapPage {}

    impl BinImpl for SwMapPage {}
}

glib::wrapper! {
    pub struct SwMapPage(ObjectSubclass<imp::SwMapPage>)
        @extends gtk::Widget, adw::Bin;
}

impl SwMapPage {
    pub fn refresh_data(&self) {
        let client = SwClient::new();

        let request = StationRequest {
            has_geo_info: Some(true),
            ..StationRequest::default()
        };

        // TODO: error handling
        client.send_station_request(request);
    }
}
