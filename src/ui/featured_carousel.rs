// Shortwave - featured_carousel.rs
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
use adw::Carousel;
use glib::clone;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use std::cell::RefCell;
use std::str::FromStr;

#[derive(Debug)]
pub struct Page {
    page: gtk::Box,
    color: gdk::RGBA,
}

mod imp {
    use super::*;
    use glib::subclass;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/featured_carousel.ui")]
    pub struct SwFeaturedCarousel {
        #[template_child]
        pub overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub carousel: TemplateChild<Carousel>,
        #[template_child]
        pub previous_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_button: TemplateChild<gtk::Button>,
        pub pages: RefCell<Vec<Page>>,
        pub css_provider: gtk::CssProvider,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwFeaturedCarousel {
        const NAME: &'static str = "SwFeaturedCarousel";
        type ParentType = adw::Bin;
        type Type = super::SwFeaturedCarousel;

        fn new() -> Self {
            let pages = RefCell::new(Vec::new());
            let css_provider = gtk::CssProvider::new();

            Self {
                overlay: TemplateChild::default(),
                carousel: TemplateChild::default(),
                previous_button: TemplateChild::default(),
                next_button: TemplateChild::default(),
                pages,
                css_provider,
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwFeaturedCarousel {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.init();
        }
    }

    impl WidgetImpl for SwFeaturedCarousel {}

    impl BinImpl for SwFeaturedCarousel {}
}

glib::wrapper! {
    pub struct SwFeaturedCarousel(ObjectSubclass<imp::SwFeaturedCarousel>)
        @extends gtk::Widget, adw::Bin;
}

impl SwFeaturedCarousel {
    pub fn init(&self) {
        let imp = imp::SwFeaturedCarousel::from_instance(self);

        let style_ctx = imp.carousel.style_context();
        style_ctx.add_provider(&imp.css_provider, 600);

        self.setup_signals();
    }

    pub fn add_page(&self, title: &str, color: &str, action: Option<Action>) {
        let imp = imp::SwFeaturedCarousel::from_instance(self);

        let builder = gtk::Builder::from_resource("/de/haeckerfelix/Shortwave/gtk/featured_carousel_page.ui");
        get_widget!(builder, gtk::Box, page_box);
        get_widget!(builder, gtk::Label, title_label);
        get_widget!(builder, gtk::Label, action_label);
        get_widget!(builder, gtk::Button, action_button);

        title_label.set_text(title);

        if let Some(a) = action {
            action_button.set_visible(true);
            action_button.set_action_name(Some(&a.name));
            action_label.set_text(&a.label);
        }

        imp.carousel.append(&page_box);

        let rgba = gdk::RGBA::from_str(color).unwrap();
        let page = Page { page: page_box, color: rgba };

        imp.pages.borrow_mut().append(&mut vec![page]);

        if imp.pages.borrow().len() == 1 {
            self.update_style();
        }

        self.update_buttons();
    }

    fn setup_signals(&self) {
        let imp = imp::SwFeaturedCarousel::from_instance(self);

        imp.previous_button.connect_clicked(clone!(@weak self as this => move |_|{
            let imp = imp::SwFeaturedCarousel::from_instance(&this);
            let position = imp.carousel.position().round() as usize;

            if position > 0 {
                imp.carousel.scroll_to(&imp.pages.borrow()[position - 1].page, true);
            }else{
                imp.carousel.scroll_to(&imp.pages.borrow()[0].page, true);
            }
        }));

        imp.next_button.connect_clicked(clone!(@weak self as this => move |_|{
            let imp = imp::SwFeaturedCarousel::from_instance(&this);
            let position = imp.carousel.position().round() as usize;

            if position < imp.pages.borrow().len() - 1 {
                imp.carousel.scroll_to(&imp.pages.borrow()[position + 1].page, true);
            }else{
                imp.carousel.scroll_to(&imp.pages.borrow()[(imp.pages.borrow().len() - 1)].page, true);
            }
        }));

        imp.carousel.connect_position_notify(clone!(@weak self as this => move |_|{
            this.update_buttons();
            this.update_style();
        }));
    }

    fn update_buttons(&self) {
        let imp = imp::SwFeaturedCarousel::from_instance(self);

        let position = imp.carousel.position();
        let length = (imp.pages.borrow().len() - 1) as f64;

        imp.previous_button.set_can_target(position > 0.0);
        imp.previous_button.set_opacity(position.min(1.0));

        imp.next_button.set_can_target(position < length);
        imp.next_button.set_opacity((length - position).min(1.0));
    }

    fn update_style(&self) {
        let imp = imp::SwFeaturedCarousel::from_instance(self);

        if imp.pages.borrow().len() == 0 {
            return;
        }

        let position = imp.carousel.position();
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;

        if lower == upper {
            let round = position.round() as usize;
            let color = imp.pages.borrow()[round].color;

            self.set_color(&color);
            return;
        }

        let color1 = imp.pages.borrow()[lower].color;
        let color2 = imp.pages.borrow()[upper].color;
        let progress = (position - lower as f64) as f32;

        let color = gdk::RGBA::new(
            color1.red() * (1.0 - progress) + color2.red() * progress,
            color1.green() * (1.0 - progress) + color2.green() * progress,
            color1.blue() * (1.0 - progress) + color2.blue() * progress,
            1.0,
        );

        self.set_color(&color);
    }

    fn set_color(&self, color: &gdk::RGBA) {
        let imp = imp::SwFeaturedCarousel::from_instance(self);

        imp.css_provider.load_from_data(
            format!(
                "carousel {{
                  background-color: {};
                }}",
                &color.to_string(),
            )
            .as_bytes(),
        );

        // Copied from gtk/gtkcolorswatch.c, INTENSITY() macro
        let intensity = color.red() * 0.30 + color.green() * 0.59 + color.blue() * 0.11;
        if intensity > 0.5 {
            imp.overlay.add_css_class("dark-foreground");
        } else {
            imp.overlay.remove_css_class("dark-foreground");
        }
    }
}

pub struct Action {
    pub name: String,
    pub label: String,
}

impl Action {
    pub fn new(name: &str, label: &str) -> Self {
        Self {
            name: name.to_owned(),
            label: label.to_owned(),
        }
    }
}
