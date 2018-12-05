extern crate gdk;
extern crate gio;
extern crate glib;
extern crate gtk;

use gio::prelude::*;
use gtk::prelude::*;

use rustio::{Station};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};

use library::Library;
use player::{PlaybackState, Player};
use search::Search;
use window::{View, Window};
use station_model::{Sorting, Order};

#[derive(Debug, Clone)]
pub enum Action {
    ViewShowSearch,
    ViewShowLibrary,
    ViewShowCurrentPlayback,
    ViewRaise,
    ViewSetSorting(Sorting, Order),
    PlaybackSetStation(Station),
    PlaybackStart,
    PlaybackStop,
    LibraryWrite,
    LibraryImport,
    LibraryExport,
    LibraryAddStations(Vec<Station>),
    LibraryRemoveStations(Vec<Station>),
}

#[derive(Clone)]
pub struct AppInfo {
    pub version: String,
    pub profile: String,
    pub app_name: String,
    pub app_id: String,
}

pub struct App {
    info: AppInfo,
    gtk_app: gtk::Application,

    sender: Sender<Action>,
    receiver: Receiver<Action>,

    window: Window,
    player: Player,
    library: Library,
    search: Search,
}

impl App {
    pub fn new() -> Rc<Self> {
        let info = AppInfo {
            version:  option_env!("VERSION").unwrap_or("0.0.0").to_string(),
            profile: option_env!("PROFILE").unwrap_or("default").to_string(),
            app_name: "Radio".to_string(),
            app_id: option_env!("APP_ID").unwrap_or("de.haeckerfelix.Gradio").to_string(),
        };

        // Set custom style
        let p = gtk::CssProvider::new();
        gtk::CssProvider::load_from_resource(&p, "/de/haeckerfelix/Gradio/gtk/style.css");
        gtk::StyleContext::add_provider_for_screen(&gdk::Screen::get_default().unwrap(), &p, 500);

        let gtk_app = gtk::Application::new(info.app_id.as_str(), gio::ApplicationFlags::FLAGS_NONE).unwrap();
        let (sender, receiver) = channel();

        let window = Window::new(sender.clone(), &info);
        let player = Player::new(sender.clone(), &info);
        let library = Library::new(sender.clone(), &info);
        let search = Search::new(sender.clone());

        window.player_box.add(&player.widget);
        window.library_box.add(&library.widget);
        window.search_box.add(&search.widget);

        let app = Rc::new(Self {
            info,
            gtk_app,
            sender,
            receiver,
            window,
            player,
            library,
            search,
        });

        glib::set_application_name(&app.info.app_name);
        glib::set_prgname(Some("gradio"));
        gtk::Window::set_default_icon_name(&app.info.app_id);

        app.setup_gaction();
        app.setup_signals();
        app
    }

    pub fn run(&self, app: Rc<Self>) {
        info!("{} ({})", self.info.app_name, self.info.app_id);
        info!("Version: {} ({})", self.info.version, self.info.profile);

        let a = app.clone();
        gtk::timeout_add(25, move || a.process_action());

        self.gtk_app.run(&[]);
        self.library.write_data();
    }

    fn setup_gaction(&self) {
        // Quit
        let gtk_app = self.gtk_app.clone();
        self.add_gaction("quit", move |_, _| gtk_app.quit());
        self.gtk_app.set_accels_for_action("app.quit", &["<primary>q"]);

        // Shortcuts
        let window = self.window.widget.clone();
        self.add_gaction("shortcuts", move |_, _| {
            Self::show_shortcuts_dialog(window.clone());
        });

        // About
        let window = self.window.widget.clone();
        let info = self.info.clone();
        self.add_gaction("about", move |_, _| {
            Self::show_about_dialog(info.clone(), window.clone());
        });

        // Save library
        let sender = self.sender.clone();
        self.add_gaction("save", move |_, _| {
            sender.send(Action::LibraryWrite).unwrap();
        });
        self.gtk_app.set_accels_for_action("app.save", &["<primary>s"]);

        // Import library
        let sender = self.sender.clone();
        self.add_gaction("import-library", move |_, _| {
            sender.send(Action::LibraryImport).unwrap();
        });

        // Export library
        let sender = self.sender.clone();
        self.add_gaction("export-library", move |_, _| {
            sender.send(Action::LibraryExport).unwrap();
        });

        // Sort / Order menu
        let sort_variant = "name".to_variant();
        let sorting_action = gio::SimpleAction::new_stateful("sorting", sort_variant.type_(), &sort_variant);
        self.gtk_app.add_action(&sorting_action);

        let order_variant = "ascending".to_variant();
        let order_action = gio::SimpleAction::new_stateful("order", order_variant.type_(), &order_variant);
        self.gtk_app.add_action(&order_action);

        let sa = sorting_action.clone();
        let oa = order_action.clone();
        let sender = self.sender.clone();
        sorting_action.connect_activate(move |a,b| {
            a.set_state(&b.clone().unwrap());
            Self::sort_action(&sa, &oa, &sender);
        });

        let sa = sorting_action.clone();
        let oa = order_action.clone();
        let sender = self.sender.clone();
        order_action.connect_activate(move |a,b| {
            a.set_state(&b.clone().unwrap());
            Self::sort_action(&sa, &oa, &sender);
        });
    }

    fn sort_action(sorting_action: &gio::SimpleAction, order_action: &gio::SimpleAction, sender: &Sender<Action>){
        let order_str: String = order_action.get_state().unwrap().get_str().unwrap().to_string();
        let order = match order_str.as_ref(){
            "ascending" => Order::Ascending,
            _ => Order::Descending,
        };

        let sorting_str: String = sorting_action.get_state().unwrap().get_str().unwrap().to_string();
        let sorting = match sorting_str.as_ref(){
            "language" => Sorting::Language,
            "country" => Sorting::Country,
            "state" => Sorting::State,
            "codec" => Sorting::Codec,
            "votes" => Sorting::Votes,
            "bitrate" => Sorting::Bitrate,
            _ => Sorting::Name,
        };

        debug!("Sorting: {} / {}", sorting_str, order_str);
        sender.send(Action::ViewSetSorting(sorting, order)).unwrap();
    }

    fn add_gaction<F>(&self, name: &str, action: F)
    where
        for<'r, 's> F: Fn(&'r gio::SimpleAction, &'s Option<glib::Variant>) + 'static,
    {
        let simple_action = gio::SimpleAction::new(name, None);
        simple_action.connect_activate(action);
        self.gtk_app.add_action(&simple_action);
    }

    fn setup_signals(&self) {
        let window = self.window.widget.clone();
        self.gtk_app.connect_activate(move |app| app.add_window(&window));
    }

    fn process_action(&self) -> glib::Continue {
        if let Ok(action) = self.receiver.try_recv() {
            debug!("Incoming action: {:?}", action);
            match action {
                Action::ViewShowSearch => self.window.set_view(View::Search),
                Action::ViewShowLibrary => self.window.set_view(View::Library),
                Action::ViewShowCurrentPlayback => self.window.set_view(View::CurrentPlayback),
                Action::ViewRaise => self.window.widget.present_with_time((glib::get_monotonic_time() / 1000) as u32),
                Action::ViewSetSorting(sorting, order) => self.library.set_sorting(sorting, order),
                Action::PlaybackSetStation(station) => self.player.set_station(station),
                Action::PlaybackStart => self.player.set_playback(PlaybackState::Playing),
                Action::PlaybackStop => self.player.set_playback(PlaybackState::Stopped),
                Action::LibraryWrite => self.library.write_data(),
                Action::LibraryImport => self.import_library(),
                Action::LibraryExport => self.export_library(),
                Action::LibraryAddStations(stations) => self.library.add_stations(stations),
                Action::LibraryRemoveStations(stations) => self.library.remove_stations(stations),
            }
        }
        glib::Continue(true)
    }

    fn show_about_dialog(info: AppInfo, window: gtk::ApplicationWindow) {
        let dialog = gtk::AboutDialog::new();
        dialog.set_program_name(info.app_name.as_str());
        dialog.set_logo_icon_name(info.app_id.as_str());
        dialog.set_comments("A web radio client");
        dialog.set_copyright("© 2018 Felix Häcker");
        dialog.set_license_type(gtk::License::Gpl30);
        dialog.set_version(info.version.as_str());
        dialog.set_transient_for(&window);
        dialog.set_modal(true);

        dialog.set_authors(&["Felix Häcker"]);
        dialog.set_artists(&["Tobias Bernard"]);

        dialog.connect_response(|dialog, _| dialog.destroy());
        dialog.show();
    }

    // TODO: This should be done with https://valadoc.org/gtk+-3.0/Gtk.ApplicationWindow.set_help_overlay.html
    fn show_shortcuts_dialog(window: gtk::ApplicationWindow) {
        let builder = gtk::Builder::new_from_resource("/de/haeckerfelix/Gradio/gtk/shortcuts.ui");
        let dialog: gtk::ShortcutsWindow = builder.get_object("shortcuts").unwrap();

        dialog.set_transient_for(&window);
        dialog.set_modal(true);
        dialog.show_all();
    }

    fn import_library(&self) {
        let import_dialog = gtk::FileChooserNative::new("Select database to import", &self.window.widget, gtk::FileChooserAction::Open, "Import", "Cancel");
        let filter = gtk::FileFilter::new();
        import_dialog.set_filter(&filter);
        filter.add_mime_type("application/x-sqlite3");

        if gtk::ResponseType::from(import_dialog.run()) == gtk::ResponseType::Accept {
            let path = import_dialog.get_file().unwrap().get_path().unwrap();
            debug!("Import path: {:?}", path);
            match self.library.import_from_path(&path) {
                Ok(_) => info!("Successfully imported library"),
                Err(err) => {
                    let dialog = gtk::MessageDialog::new(
                        Some(&self.window.widget),
                        gtk::DialogFlags::DESTROY_WITH_PARENT,
                        gtk::MessageType::Info,
                        gtk::ButtonsType::Close,
                        &format!("Could not import library:\n\n{:?}", err),
                    );
                    dialog.run();
                    dialog.destroy();
                }
            };
        }
        import_dialog.destroy();
    }

    fn export_library(&self) {
        let export_dialog = gtk::FileChooserNative::new("Export database", &self.window.widget, gtk::FileChooserAction::Save, "Export", "Cancel");
        if gtk::ResponseType::from(export_dialog.run()) == gtk::ResponseType::Accept {
            let path = export_dialog.get_file().unwrap().get_path().unwrap();
            debug!("Export path: {:?}", path);
            match self.library.export_to_path(&path) {
                Ok(_) => info!("Successfully exported library"),
                Err(err) => {
                    let dialog = gtk::MessageDialog::new(
                        Some(&self.window.widget),
                        gtk::DialogFlags::DESTROY_WITH_PARENT,
                        gtk::MessageType::Info,
                        gtk::ButtonsType::Close,
                        &format!("Could not export library:\n\n{:?}", err),
                    );
                    dialog.run();
                    dialog.destroy();
                }
            };
        }
        export_dialog.destroy();
    }
}
