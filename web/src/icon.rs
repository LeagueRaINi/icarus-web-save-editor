use leptos::prelude::*;

/// Renders a decoded game icon, or a subtle placeholder box if none was
/// extracted (some assets use pixel formats/paths we don't resolve, or the
/// talent just has no icon at all).
#[component]
pub fn Icon(
    file: Option<String>,
    #[prop(default = 28)] size: u32,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    match file {
        Some(f) => view! {
            <img
                class=format!("icon {class}")
                src=format!("/icons/{f}")
                width=size
                height=size
                loading="lazy"
                decoding="async"
                alt=""
            />
        }
            .into_any(),
        None => view! {
            <span
                class=format!("icon icon-placeholder {class}")
                style=format!("width:{size}px;height:{size}px")
            ></span>
        }
            .into_any(),
    }
}
