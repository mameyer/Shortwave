// Shortwave - station_dialog.rs
// Copyright (C) 2021  Felix Häcker <haeckerfelix@gnome.org>
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

use crate::api::{StationMetadata, SwStation};
use crate::app::{Action, SwApplication};
use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use glib::Sender;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gio, glib};
use once_cell::unsync::OnceCell;
use url::Url;
use uuid::Uuid;

mod imp {
    use super::*;
    use glib::subclass;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/new_station_dialog.ui")]
    pub struct SwNewStationDialog {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub selection_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub create_online_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub create_local_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub editor_page: TemplateChild<gtk::Box>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub add_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub name_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub url_entry: TemplateChild<gtk::Entry>,

        pub sender: OnceCell<Sender<Action>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwNewStationDialog {
        const NAME: &'static str = "SwNewStationDialog";
        type ParentType = adw::Window;
        type Type = super::SwNewStationDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.install_action("dialog.close", None, |this, _, _| {
                this.hide();
                this.close();
            });

            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwNewStationDialog {}

    impl WidgetImpl for SwNewStationDialog {}

    impl WindowImpl for SwNewStationDialog {}

    impl AdwWindowImpl for SwNewStationDialog {}
}

glib::wrapper! {
    pub struct SwNewStationDialog(ObjectSubclass<imp::SwNewStationDialog>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

impl SwNewStationDialog {
    pub fn new(sender: Sender<Action>) -> Self {
        let dialog = glib::Object::new(&[]).unwrap();

        let imp = imp::SwNewStationDialog::from_instance(&dialog);
        imp.sender.set(sender).unwrap();

        let window = gio::Application::default().unwrap().downcast_ref::<SwApplication>().unwrap().active_window().unwrap();
        dialog.set_transient_for(Some(&window));

        dialog.setup_signals();
        dialog
    }

    fn setup_signals(&self) {
        let imp = imp::SwNewStationDialog::from_instance(self);

        imp.create_online_row.connect_activated(clone!(@weak self as this => move |_| {
            open::that("https://www.radio-browser.info/#/add").expect("Could not open webpage.");
            this.close();
        }));

        imp.create_local_row.connect_activated(clone!(@weak self as this => move |_| {
            let imp = imp::SwNewStationDialog::from_instance(&this);
            imp.stack.set_visible_child(&imp.editor_page.get());
        }));

        imp.back_button.connect_clicked(clone!(@weak self as this => move |_| {
            let imp = imp::SwNewStationDialog::from_instance(&this);
            imp.stack.set_visible_child(&imp.selection_page.get());
        }));

        imp.add_button.connect_clicked(clone!(@weak self as this => move |_| {
            let imp = imp::SwNewStationDialog::from_instance(&this);

            let uuid = Uuid::new_v4().to_string();
            let name = imp.name_entry.text().to_string();
            let url = Url::parse(&imp.url_entry.text()).unwrap();

            let station = SwStation::new(uuid, true, StationMetadata::new(name, url));
            send!(imp.sender.get().unwrap(), Action::LibraryAddStations(vec![station]));
            this.close();
        }));

        imp.name_entry.connect_changed(clone!(@weak self as this => move |_| {
            this.validate();
        }));

        imp.url_entry.connect_changed(clone!(@weak self as this => move |_| {
            this.validate();
        }));
    }

    fn validate(&self) {
        let imp = imp::SwNewStationDialog::from_instance(self);

        let have_name = !imp.name_entry.text().is_empty();
        let url = imp.url_entry.text().to_string();

        match Url::parse(&url) {
            Ok(_) => {
                imp.url_entry.remove_css_class("error");
                imp.add_button.set_sensitive(have_name);
            }
            Err(_) => {
                imp.url_entry.add_css_class("error");
                imp.add_button.set_sensitive(false);
            }
        }
    }
}
