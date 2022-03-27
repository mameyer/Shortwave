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

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use glib::clone;
use glib::Sender;
use gtk::gdk_pixbuf;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gio, glib};
use once_cell::unsync::OnceCell;
use url::Url;
use uuid::Uuid;

use crate::api::{StationMetadata, SwStation};
use crate::app::{Action, SwApplication};
use crate::ui::{FaviconSize, StationFavicon};

mod imp {
    use std::cell::RefCell;

    use super::*;
    use glib::subclass;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/create_station_dialog.ui")]
    pub struct SwCreateStationDialog {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub create_online_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub create_local_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub create_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub favicon_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub favicon_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub name_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub url_entry: TemplateChild<gtk::Entry>,

        pub favicon: RefCell<Option<gdk_pixbuf::Pixbuf>>,
        pub favicon_widget: OnceCell<StationFavicon>,
        pub file_chooser: OnceCell<gtk::FileChooserNative>,
        pub sender: OnceCell<Sender<Action>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwCreateStationDialog {
        const NAME: &'static str = "SwCreateStationDialog";
        type ParentType = adw::Window;
        type Type = super::SwCreateStationDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.install_action("dialog.close", None, |this, _, _| {
                this.hide();
                this.close();
            });

            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwCreateStationDialog {}

    impl WidgetImpl for SwCreateStationDialog {}

    impl WindowImpl for SwCreateStationDialog {}

    impl AdwWindowImpl for SwCreateStationDialog {}
}

glib::wrapper! {
    pub struct SwCreateStationDialog(ObjectSubclass<imp::SwCreateStationDialog>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

#[gtk::template_callbacks]
impl SwCreateStationDialog {
    pub fn new(sender: Sender<Action>) -> Self {
        let dialog: Self = glib::Object::new(&[]).unwrap();

        let imp = dialog.imp();
        let favicon_widget = StationFavicon::new(FaviconSize::Big);
        let file_chooser = gtk::FileChooserNative::builder().transient_for(&dialog).modal(true).title(&gettext("Select station image")).build();

        imp.favicon_widget.set(favicon_widget).unwrap();
        imp.favicon_box.append(&imp.favicon_widget.get().unwrap().widget);
        imp.file_chooser.set(file_chooser).unwrap();
        imp.sender.set(sender).unwrap();

        let window = gio::Application::default().unwrap().downcast_ref::<SwApplication>().unwrap().active_window().unwrap();
        dialog.set_transient_for(Some(&window));

        dialog.setup_signals();
        dialog
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        imp.back_button.connect_clicked(clone!(@weak self as this => move |_| {
            let imp = imp::SwCreateStationDialog::from_instance(&this);
            imp.stack.set_visible_child_name("start");
        }));

        imp.favicon_button.connect_clicked(clone!(@weak self as this => move |_| {
            let imp = imp::SwCreateStationDialog::from_instance(&this);
            imp.file_chooser.get().unwrap().show();
        }));

        imp.file_chooser.get().unwrap().connect_response(clone!(@weak self as this => move |file_chooser, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(file) = file_chooser.file() {
                    this.set_favicon(file);
                }
            }
        }));
    }

    #[template_callback]
    fn create_public_station(&self) {
        open::that("https://www.radio-browser.info/#/add").expect("Could not open webpage.");
        self.close();
    }

    #[template_callback]
    fn create_local_station(&self) {
        self.imp().stack.set_visible_child_name("local-station");
    }

    #[template_callback]
    fn create_station(&self) {
        let imp = self.imp();
        let uuid = Uuid::new_v4().to_string();
        let name = imp.name_entry.text().to_string();
        let url = Url::parse(&imp.url_entry.text()).unwrap();
        let favicon = imp.favicon.borrow().clone();

        let station = SwStation::new(uuid, true, StationMetadata::new(name, url), favicon);
        send!(imp.sender.get().unwrap(), Action::LibraryAddStations(vec![station]));
        self.close();
    }

    #[template_callback]
    fn validate_input(&self) {
        let imp = self.imp();

        let have_name = !imp.name_entry.text().is_empty();
        let url = imp.url_entry.text().to_string();

        match Url::parse(&url) {
            Ok(_) => {
                imp.url_entry.remove_css_class("error");
                imp.create_button.set_sensitive(have_name);
            }
            Err(_) => {
                imp.url_entry.add_css_class("error");
                imp.create_button.set_sensitive(false);
            }
        }
    }

    fn set_favicon(&self, file: gio::File) {
        if let Some(path) = file.path() {
            if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_file(&path) {
                self.imp().favicon_widget.get().unwrap().set_pixbuf(&pixbuf);
                self.imp().favicon.replace(Some(pixbuf));
            }
        }
    }
}
