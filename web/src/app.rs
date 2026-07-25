use crate::character::{parse_characters_file, serialize_characters_file, CharacterSave};
use crate::data::{CATEGORIES, WORKSHOP_CATEGORIES};
use crate::fields_panel::CharacterFieldsPanel;
use crate::file_io::{read_input_file, trigger_download};
use crate::profile::{parse_profile_file, serialize_profile_file, ProfileSave};
use crate::profile_fields::ProfileFieldsPanel;
use crate::talents_panel::TalentBrowser;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Character,
    Profile,
}

#[component]
pub fn App() -> impl IntoView {
    let mode: RwSignal<Mode> = RwSignal::new(Mode::Character);

    view! {
        <div class="app">
            <header>
                <div class="header-row">
                    <h1>"Icarus Save Editor"</h1>
                    <div class="tabs mode-tabs">
                        <button class:active=move || mode.get() == Mode::Character on:click=move |_| mode.set(Mode::Character)>
                            "Character"
                        </button>
                        <button class:active=move || mode.get() == Mode::Profile on:click=move |_| mode.set(Mode::Profile)>
                            "Profile & Workshop"
                        </button>
                    </div>
                </div>
            </header>
            <div class="backup-banner">
                "⚠ Always back up your original Characters.json / Profile.json before overwriting "
                "them with an edited copy. Close the game before replacing the file(s)."
            </div>
            // Both editors stay mounted at all times (just hidden via CSS)
            // rather than being conditionally rendered, so switching tabs
            // doesn't throw away whatever file you already loaded on the
            // other one.
            <div style:display=move || if mode.get() == Mode::Character { "block" } else { "none" }>
                <CharacterEditor />
            </div>
            <div style:display=move || if mode.get() == Mode::Profile { "block" } else { "none" }>
                <ProfileEditor />
            </div>
            <footer>
                <span>
                    "Icarus Save Editor "
                    {match option_env!("GIT_HASH") {
                        Some(hash) => format!("v{} ({hash})", env!("CARGO_PKG_VERSION")),
                        None => format!("v{}", env!("CARGO_PKG_VERSION")),
                    }}
                </span>
                <span class="footer-sep">"·"</span>
                <span>
                    "A fan-made tool. Not affiliated with or endorsed by RocketWerkz or the game ICARUS."
                </span>
            </footer>
        </div>
    }
}

/// Landing card shown before a file is loaded: what to open and where the
/// game keeps it.
#[component]
fn EmptyState(filename: &'static str, blurb: &'static str) -> impl IntoView {
    view! {
        <div class="empty-state">
            <div class="empty-state-icon">"📂"</div>
            <h2>"No file loaded"</h2>
            <p>{blurb}</p>
            <p class="empty-state-path">
                "Open " <code>{filename}</code> " from:"<br />
                <code>"%LOCALAPPDATA%\\Icarus\\Saved\\PlayerData\\<your-steam-id>\\"</code>
            </p>
            <p class="empty-state-note">
                "Tip: back up the original file before replacing it with the downloaded copy, and close the game before you replace it."
            </p>
        </div>
    }
}

#[component]
fn CharacterEditor() -> impl IntoView {
    let characters: RwSignal<Option<Vec<CharacterSave>>> = RwSignal::new(None);
    let selected: RwSignal<Option<usize>> = RwSignal::new(None);
    let status: RwSignal<String> = RwSignal::new(String::new());

    let on_file_change = move |ev: leptos::ev::Event| {
        let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else {
            return;
        };
        spawn_local(async move {
            let Some(result) = read_input_file(&input).await else {
                return;
            };
            match result {
                Ok(text) => match parse_characters_file(&text) {
                    Ok(list) => {
                        status.set(format!("Loaded {} character(s).", list.len()));
                        selected.set(if list.is_empty() { None } else { Some(0) });
                        characters.set(Some(list));
                    }
                    Err(e) => status.set(format!("Failed to parse Characters.json: {e}")),
                },
                Err(e) => status.set(format!("Failed to read file: {e}")),
            }
        });
    };

    let on_download = move |_| {
        characters.with(|opt| {
            let Some(list) = opt else { return };
            match serialize_characters_file(list) {
                Ok(text) => {
                    trigger_download("Characters.json", &text);
                    status.set("Downloaded Characters.json".to_string());
                }
                Err(e) => status.set(format!("Failed to serialize: {e}")),
            }
        });
    };

    view! {
        <div class="toolbar">
            <label class="file-picker">
                "Open Characters.json"
                <input type="file" accept=".json" on:change=on_file_change />
            </label>
            <button class="download-btn" disabled=move || characters.with(|c| c.is_none()) on:click=on_download>
                "Download edited file"
            </button>
            <span class="status">{move || status.get()}</span>
        </div>

        {move || {
            characters
                .with(|opt| opt.is_none())
                .then(|| view! {
                    <EmptyState
                        filename="Characters.json"
                        blurb="Edit character names, XP, talents and blueprints."
                    />
                })
        }}

        {move || {
            characters
                .with(|opt| opt.as_ref().is_some_and(|l| l.len() > 1))
                .then(|| {
                    view! {
                        <div class="character-tabs">
                            {move || {
                                characters
                                    .with(|opt| {
                                        opt.as_ref()
                                            .map(|list| {
                                                list.iter()
                                                    .enumerate()
                                                    .map(|(i, c)| {
                                                        let name = format!("{} (slot {})", c.character_name, c.chr_slot);
                                                        view! {
                                                            <button
                                                                class:active=move || selected.get() == Some(i)
                                                                on:click=move |_| selected.set(Some(i))
                                                            >
                                                                {name}
                                                            </button>
                                                        }
                                                    })
                                                    .collect_view()
                                            })
                                            .unwrap_or_default()
                                    })
                            }}
                        </div>
                    }
                })
        }}

        {move || {
            selected
                .get()
                .map(|_| {
                    view! {
                        <main>
                            <aside class="side-panel">
                                <CharacterFieldsPanel characters=characters selected=selected />
                            </aside>
                            <TalentBrowser
                                characters=characters
                                selected=selected
                                categories=CATEGORIES.as_slice()
                            />
                        </main>
                    }
                })
        }}
    }
}

#[component]
fn ProfileEditor() -> impl IntoView {
    // ProfileSave is a single object, but wrapping it in a length-1 Vec
    // keeps the signal shape identical to the character editor's
    // Vec<CharacterSave>, so the shared panel patterns apply unchanged.
    let profile: RwSignal<Option<Vec<ProfileSave>>> = RwSignal::new(None);
    let selected: RwSignal<Option<usize>> = RwSignal::new(None);
    let status: RwSignal<String> = RwSignal::new(String::new());

    let on_file_change = move |ev: leptos::ev::Event| {
        let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else {
            return;
        };
        spawn_local(async move {
            let Some(result) = read_input_file(&input).await else {
                return;
            };
            match result {
                Ok(text) => match parse_profile_file(&text) {
                    Ok(p) => {
                        status.set("Loaded Profile.json".to_string());
                        profile.set(Some(vec![p]));
                        selected.set(Some(0));
                    }
                    Err(e) => status.set(format!("Failed to parse Profile.json: {e}")),
                },
                Err(e) => status.set(format!("Failed to read file: {e}")),
            }
        });
    };

    let on_download = move |_| {
        profile.with(|opt| {
            let Some(list) = opt else { return };
            let Some(p) = list.first() else { return };
            match serialize_profile_file(p) {
                Ok(text) => {
                    trigger_download("Profile.json", &text);
                    status.set("Downloaded Profile.json".to_string());
                }
                Err(e) => status.set(format!("Failed to serialize: {e}")),
            }
        });
    };

    view! {
        <div class="toolbar">
            <label class="file-picker">
                "Open Profile.json"
                <input type="file" accept=".json" on:change=on_file_change />
            </label>
            <button class="download-btn" disabled=move || profile.with(|c| c.is_none()) on:click=on_download>
                "Download edited file"
            </button>
            <span class="status">{move || status.get()}</span>
        </div>

        {move || {
            profile
                .with(|opt| opt.is_none())
                .then(|| view! {
                    <EmptyState
                        filename="Profile.json"
                        blurb="Edit account-wide currencies and orbital workshop research."
                    />
                })
        }}

        {move || {
            selected
                .get()
                .map(|_| {
                    view! {
                        <main>
                            <aside class="side-panel">
                                <ProfileFieldsPanel profile=profile selected=selected />
                            </aside>
                            <TalentBrowser
                                characters=profile
                                selected=selected
                                categories=WORKSHOP_CATEGORIES.as_slice()
                            />
                        </main>
                    }
                })
        }}
    }
}
