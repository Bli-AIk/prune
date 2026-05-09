#![cfg(feature = "gui")]

use dioxus::prelude::*;

pub fn launch() {
    dioxus::LaunchBuilder::desktop().launch(app);
}

fn app() -> Element {
    rsx! {
        main {
            h1 { "Prune" }
            section { "Android build and deploy" }
        }
    }
}
