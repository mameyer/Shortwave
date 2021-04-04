// Shortwave - search_page.rs
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

use adw::subclass::prelude::*;
use glib::{clone, Sender};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{gio, glib};
use once_cell::unsync::OnceCell;

use crate::app::{Action, SwApplication};
use crate::config;
use crate::database::{SwLibrary, SwLibraryStatus};
use crate::i18n::*;
use crate::model::SwSorting;
use crate::ui::SwStationFlowBox;

mod imp {
    use super::*;
    use glib::subclass;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/library_page.ui")]
    pub struct SwLibraryPage {
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub flowbox: TemplateChild<SwStationFlowBox>,

        pub library: SwLibrary,
        pub sender: OnceCell<Sender<Action>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwLibraryPage {
        const NAME: &'static str = "SwLibraryPage";
        type ParentType = adw::Bin;
        type Type = super::SwLibraryPage;

        fn new() -> Self {
            let status_page = TemplateChild::default();
            let stack = TemplateChild::default();
            let flowbox = TemplateChild::default();

            let app = gio::Application::get_default().unwrap().downcast::<SwApplication>().unwrap();
            let library = app.get_library();

            let sender = OnceCell::default();

            Self {
                status_page,
                stack,
                flowbox,
                library,
                sender,
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwLibraryPage {}

    impl WidgetImpl for SwLibraryPage {}

    impl BinImpl for SwLibraryPage {}
}

glib::wrapper! {
    pub struct SwLibraryPage(ObjectSubclass<imp::SwLibraryPage>)
        @extends gtk::Widget, adw::Bin;
}

impl SwLibraryPage {
    pub fn init(&self, sender: Sender<Action>) {
        let imp = imp::SwLibraryPage::from_instance(self);
        imp.sender.set(sender).unwrap();

        self.setup_widgets();
        self.setup_signals();
    }

    pub fn set_sorting(&self, sorting: SwSorting, descending: bool) {
        let imp = imp::SwLibraryPage::from_instance(self);
        imp.flowbox.get().set_sorting(sorting, descending);
    }

    fn setup_widgets(&self) {
        let imp = imp::SwLibraryPage::from_instance(self);

        // Setup empty state page
        imp.status_page.set_icon_name(Some(&config::APP_ID));

        // Welcome text which gets displayed when the library is empty. "{}" is the application name.
        imp.status_page.set_title(Some(&i18n_f("Welcome to {}", &[config::NAME]).as_str()));

        // Station flowbox
        imp.flowbox.init(imp.library.get_model(), imp.sender.get().unwrap().clone());
    }

    fn setup_signals(&self) {
        let imp = imp::SwLibraryPage::from_instance(self);

        imp.library.connect_notify_local(
            Some("status"),
            clone!(@weak self as this => move |library, _|{
                let imp = imp::SwLibraryPage::from_instance(&this);

                match library.get_status(){
                    SwLibraryStatus::Loading => imp.stack.set_visible_child_name("loading"),
                    SwLibraryStatus::Empty => imp.stack.set_visible_child_name("empty"),
                    SwLibraryStatus::Content => imp.stack.set_visible_child_name("content"),
                    _ => (),
                }
            }),
        );
    }
}
