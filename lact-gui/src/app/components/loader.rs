use gtk::{glib, prelude::*};

const APP_PNG: &[u8] = include_bytes!("../../../../res/io.github.Donta1979.UnifiedGPUControl.png");

const SIZE: i32 = 192;

pub(crate) fn new() -> gtk::Picture {
    let bytes = glib::Bytes::from_static(APP_PNG);

    let texture =
        gtk::gdk::Texture::from_bytes(&bytes).expect("UGC loading icon should be loadable");

    let picture = gtk::Picture::new();

    picture.set_size_request(SIZE, SIZE);
    picture.set_can_shrink(false);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_paintable(Some(&texture));

    picture
}
