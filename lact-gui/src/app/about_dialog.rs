use crate::{APP_ID, GUI_VERSION, REPO_URL, UGC_VERSION};
use adw::prelude::AdwDialogExt;
use lact_schema::GIT_COMMIT;
use relm4::{ComponentParts, ComponentSender};

pub struct AboutDialog {
    parent: adw::ApplicationWindow,
}

#[derive(Debug)]
pub enum AboutDialogMsg {
    Show,
}

#[relm4::component(pub)]
impl relm4::Component for AboutDialog {
    type Init = adw::ApplicationWindow;
    type Input = AboutDialogMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        adw::AboutDialog {
            set_application_name: "Unified GPU Control",
            set_application_icon: APP_ID,
            set_version: UGC_VERSION,
            set_developer_name: "Donta1979",
            set_comments: &format!(
                "Unified GPU monitoring, tuning, Astral Guard and MangoHud configuration.\n\nBased on LACT {GUI_VERSION} ({GIT_COMMIT})."
            ),
            set_developers: &[
                "Donta1979 - Unified GPU Control",
                "Ilya Zlobintsev - Original LACT developer",
                "LACT contributors",
            ],
            set_copyright: "Copyright © 2023 Ilya Zlobintsev and LACT contributors",
            set_website: REPO_URL,
            set_license_type: gtk::License::MitX11,
        }
    }

    fn init(
        parent: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AboutDialog { parent };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            AboutDialogMsg::Show => {
                root.present(Some(&self.parent));
            }
        }
        self.update_view(widgets, sender);
    }
}
