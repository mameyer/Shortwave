// Shortwave - station.rs
// Copyright (C) 2021-2022  Felix Häcker <haeckerfelix@gnome.org>
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

use glib::{
    ParamSpec, ParamSpecBoolean, ParamSpecBoxed, ParamSpecObject, ParamSpecString, ToValue,
};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk_pixbuf, glib};
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;

use crate::api::StationMetadata;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct SwStation {
        pub uuid: OnceCell<String>,
        pub is_local: OnceCell<bool>,
        pub is_orphaned: OnceCell<bool>,
        pub metadata: OnceCell<StationMetadata>,
        pub favicon: OnceCell<gdk_pixbuf::Pixbuf>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStation {
        const NAME: &'static str = "SwStation";
        type Type = super::SwStation;
    }

    impl ObjectImpl for SwStation {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("uuid").read_only().build(),
                    ParamSpecBoolean::builder("is-local").read_only().build(),
                    ParamSpecBoolean::builder("is-orphaned").read_only().build(),
                    ParamSpecBoxed::builder::<StationMetadata>("metadata")
                        .read_only()
                        .build(),
                    ParamSpecObject::builder::<gdk_pixbuf::Pixbuf>("favicon")
                        .read_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> glib::Value {
            match pspec.name() {
                "uuid" => self.obj().uuid().to_value(),
                "is-local" => self.obj().is_local().to_value(),
                "is-orphaned" => self.obj().is_local().to_value(),
                "metadata" => self.obj().metadata().to_value(),
                "favicon" => self.obj().favicon().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct SwStation(ObjectSubclass<imp::SwStation>);
}

impl SwStation {
    pub fn new(
        uuid: String,
        is_local: bool,
        is_orphaned: bool,
        metadata: StationMetadata,
        favicon: Option<gdk_pixbuf::Pixbuf>,
    ) -> Self {
        let station = glib::Object::new::<Self>();

        let imp = station.imp();
        imp.uuid.set(uuid).unwrap();
        imp.is_local.set(is_local).unwrap();
        imp.is_orphaned.set(is_orphaned).unwrap();
        imp.metadata.set(metadata).unwrap();

        if let Some(pixbuf) = favicon {
            imp.favicon.set(pixbuf).unwrap();
        }

        station
    }

    pub fn uuid(&self) -> String {
        self.imp().uuid.get().unwrap().clone()
    }

    pub fn is_local(&self) -> bool {
        *self.imp().is_local.get().unwrap()
    }

    pub fn is_orphaned(&self) -> bool {
        *self.imp().is_orphaned.get().unwrap()
    }

    pub fn metadata(&self) -> StationMetadata {
        self.imp().metadata.get().unwrap().clone()
    }

    pub fn favicon(&self) -> Option<gdk_pixbuf::Pixbuf> {
        self.imp().favicon.get().cloned()
    }
}
