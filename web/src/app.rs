use crate::character::{parse_characters_file, serialize_characters_file, CharacterSave};
use crate::data::{CATEGORIES, WORKSHOP_CATEGORIES};
use crate::fields_panel::CharacterFieldsPanel;
use crate::file_io::{read_input_file, trigger_download};
use crate::profile::{parse_profile_file, serialize_profile_file, ProfileSave};
use crate::profile_fields::ProfileFieldsPanel;
use crate::talents_panel::TalentBrowser;
use codee::string::FromToStringCodec;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_use::storage::use_session_storage;
use wasm_bindgen::JsCast;

/// Session-storage keys for the in-progress file of each editor. Session
/// storage is per-tab and cleared when the tab closes, so an edited save
/// survives an accidental refresh without outliving the session it was
/// opened in. What is stored is the serialized save file itself, exactly as
/// the download button would write it, so restoring is the normal parse path.
const CHARACTERS_CACHE_KEY: &str = "icarus-save-editor:characters";
const PROFILE_CACHE_KEY: &str = "icarus-save-editor:profile";

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

/// Offer to pick up where the last page load left off. Shown only when
/// nothing is open yet and this tab's session has a cached file, so it
/// never gets in the way of normal use.
#[component]
fn RestoreBanner<F, G>(summary: String, on_restore: F, on_discard: G) -> impl IntoView
where
    F: Fn() + 'static,
    G: Fn() + 'static,
{
    view! {
        <div class="restore-banner">
            <span class="restore-text">
                "Unsaved work from this session was found — " {summary} "."
            </span>
            <div class="restore-actions">
                <button class="restore-primary" on:click=move |_| on_restore()>
                    "Continue with previous data"
                </button>
                <button on:click=move |_| on_discard()>"Discard"</button>
            </div>
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

    let (cached, set_cached, clear_cached) =
        use_session_storage::<String, FromToStringCodec>(CHARACTERS_CACHE_KEY);

    // Read once, before any edit can overwrite it: whatever this tab was
    // working on when the page last unloaded. A corrupt or empty entry just
    // means no offer to restore.
    let restorable: RwSignal<Option<Vec<CharacterSave>>> =
        RwSignal::new(parse_characters_file(&cached.get_untracked()).ok().filter(|l| !l.is_empty()));

    // Mirror every edit back into storage. Skipped while nothing is open,
    // which is what keeps a pending restore offer intact until it is either
    // taken or discarded.
    Effect::new(move |_| {
        characters.with(|opt| {
            let Some(list) = opt else { return };
            if let Ok(text) = serialize_characters_file(list) {
                set_cached.set(text);
            }
        });
    });

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
            (characters.with(|opt| opt.is_none()) && restorable.with(|r| r.is_some()))
                .then(|| {
                    let summary = restorable.with(|r| {
                        let n = r.as_ref().map(|l| l.len()).unwrap_or(0);
                        format!("{n} character(s)")
                    });
                    let clear_cached = clear_cached.clone();
                    view! {
                        <RestoreBanner
                            summary=summary
                            on_restore=move || {
                                if let Some(list) = restorable.get_untracked() {
                                    status.set(format!("Restored {} character(s) from this session.", list.len()));
                                    selected.set(Some(0));
                                    characters.set(Some(list));
                                }
                                restorable.set(None);
                            }
                            on_discard=move || {
                                clear_cached();
                                restorable.set(None);
                            }
                        />
                    }
                })
        }}

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

    let (cached, set_cached, clear_cached) =
        use_session_storage::<String, FromToStringCodec>(PROFILE_CACHE_KEY);

    let restorable: RwSignal<Option<ProfileSave>> =
        RwSignal::new(parse_profile_file(&cached.get_untracked()).ok());

    Effect::new(move |_| {
        profile.with(|opt| {
            let Some(p) = opt.as_ref().and_then(|list| list.first()) else { return };
            if let Ok(text) = serialize_profile_file(p) {
                set_cached.set(text);
            }
        });
    });

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
            (profile.with(|opt| opt.is_none()) && restorable.with(|r| r.is_some()))
                .then(|| {
                    let clear_cached = clear_cached.clone();
                    view! {
                        <RestoreBanner
                            summary="an edited Profile.json".to_string()
                            on_restore=move || {
                                if let Some(p) = restorable.get_untracked() {
                                    status.set("Restored Profile.json from this session.".to_string());
                                    profile.set(Some(vec![p]));
                                    selected.set(Some(0));
                                }
                                restorable.set(None);
                            }
                            on_discard=move || {
                                clear_cached();
                                restorable.set(None);
                            }
                        />
                    }
                })
        }}

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
