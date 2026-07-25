use crate::character::CharacterSave;
use crate::levels::{estimate, spent_points, MISSION_TALENT_BONUS_MAX};
use leptos::prelude::*;

#[component]
pub fn CharacterFieldsPanel(
    characters: RwSignal<Option<Vec<CharacterSave>>>,
    selected: RwSignal<Option<usize>>,
) -> impl IntoView {
    let get_str = move |f: fn(&CharacterSave) -> String| -> String {
        characters
            .with(|opt| {
                opt.as_ref()
                    .zip(selected.get())
                    .and_then(|(list, idx)| list.get(idx))
                    .map(f)
            })
            .unwrap_or_default()
    };
    let get_i64 = move |f: fn(&CharacterSave) -> i64| -> i64 {
        characters
            .with(|opt| {
                opt.as_ref()
                    .zip(selected.get())
                    .and_then(|(list, idx)| list.get(idx))
                    .map(f)
            })
            .unwrap_or_default()
    };
    let get_bool = move |f: fn(&CharacterSave) -> bool| -> bool {
        characters
            .with(|opt| {
                opt.as_ref()
                    .zip(selected.get())
                    .and_then(|(list, idx)| list.get(idx))
                    .map(f)
            })
            .unwrap_or_default()
    };

    view! {
        <fieldset class="character-fields">
            <legend>"Character"</legend>
            <div class="field-grid">
                <label>
                    "Name"
                    <input
                        type="text"
                        prop:value=move || get_str(|c| c.character_name.clone())
                        on:input=move |ev| {
                            let v = event_target_value(&ev);
                            characters
                                .update(|opt| {
                                    if let Some(list) = opt {
                                        if let Some(idx) = selected.get() {
                                            if let Some(c) = list.get_mut(idx) {
                                                c.character_name = v;
                                            }
                                        }
                                    }
                                });
                        }
                    />
                </label>
                <label>
                    "Slot"
                    <input type="number" prop:value=move || get_i64(|c| c.chr_slot) readonly=true />
                </label>
                <label>
                    "XP"
                    <input
                        type="number"
                        prop:value=move || get_i64(|c| c.xp)
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                characters
                                    .update(|opt| {
                                        if let Some(list) = opt {
                                            if let Some(idx) = selected.get() {
                                                if let Some(c) = list.get_mut(idx) {
                                                    c.xp = v;
                                                }
                                            }
                                        }
                                    });
                            }
                        }
                    />
                </label>
                {move || {
                    characters.with(|opt| {
                        let c = opt.as_ref().zip(selected.get()).and_then(|(l, i)| l.get(i))?;
                        let est = estimate(c.xp);
                        let spent = spent_points(&c.talents);
                        // Missions grant account-wide bonus talent points on
                        // top of levelling, so only flag clear excess. Solo
                        // talents draw from their own pool and are listed
                        // separately.
                        let talent_over = spent.talent > est.talent_points + MISSION_TALENT_BONUS_MAX;
                        let talent_bonus = !talent_over && spent.talent > est.talent_points;
                        let bp_over = spent.blueprint > est.blueprint_points;
                        Some(view! {
                            <div class="level-estimate">
                                <div class="level-line">
                                    "Level (from XP): "
                                    <strong>{est.level}</strong>
                                </div>
                                <div class="level-line" class=("points-over", talent_over)>
                                    "Talent points: " {spent.talent} " / " {est.talent_points} " from levels"
                                    {talent_over.then_some(" — exceeds levels + mission rewards")}
                                    {talent_bonus.then_some(" (extra from mission rewards)")}
                                </div>
                                {(spent.solo > 0)
                                    .then(|| {
                                        let solo_over = spent.solo > est.solo_points;
                                        view! {
                                            <div class="level-line" class=("points-over", solo_over)>
                                                "Solo talent points: " {spent.solo} " / " {est.solo_points}
                                                {solo_over.then_some(" — exceeds what this level can earn")}
                                            </div>
                                        }
                                    })}
                                <div class="level-line" class=("points-over", bp_over)>
                                    "Blueprint points: " {spent.blueprint} " / " {est.blueprint_points}
                                    {bp_over.then_some(" — exceeds what this level can earn")}
                                </div>
                            </div>
                        })
                    })
                }}
                <label>
                    "XP Debt"
                    <input
                        type="number"
                        prop:value=move || get_i64(|c| c.xp_debt)
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                characters
                                    .update(|opt| {
                                        if let Some(list) = opt {
                                            if let Some(idx) = selected.get() {
                                                if let Some(c) = list.get_mut(idx) {
                                                    c.xp_debt = v;
                                                }
                                            }
                                        }
                                    });
                            }
                        }
                    />
                </label>
                <label>
                    "Location"
                    <input
                        type="text"
                        prop:value=move || get_str(|c| c.location.clone().unwrap_or_default())
                        on:input=move |ev| {
                            let v = event_target_value(&ev);
                            characters
                                .update(|opt| {
                                    if let Some(list) = opt {
                                        if let Some(idx) = selected.get() {
                                            if let Some(c) = list.get_mut(idx) {
                                                c.location = Some(v);
                                            }
                                        }
                                    }
                                });
                        }
                    />
                </label>
                <label>
                    "Last Prospect"
                    <input
                        type="text"
                        prop:value=move || get_str(|c| c.last_prospect_id.clone().unwrap_or_default())
                        on:input=move |ev| {
                            let v = event_target_value(&ev);
                            characters
                                .update(|opt| {
                                    if let Some(list) = opt {
                                        if let Some(idx) = selected.get() {
                                            if let Some(c) = list.get_mut(idx) {
                                                c.last_prospect_id = Some(v);
                                            }
                                        }
                                    }
                                });
                        }
                    />
                </label>
                <label class="checkbox-field">
                    <input
                        type="checkbox"
                        prop:checked=move || get_bool(|c| c.is_dead)
                        on:change=move |ev| {
                            let v = event_target_checked(&ev);
                            characters
                                .update(|opt| {
                                    if let Some(list) = opt {
                                        if let Some(idx) = selected.get() {
                                            if let Some(c) = list.get_mut(idx) {
                                                c.is_dead = v;
                                            }
                                        }
                                    }
                                });
                        }
                    />
                    "Dead"
                </label>
                <label class="checkbox-field">
                    <input
                        type="checkbox"
                        prop:checked=move || get_bool(|c| c.is_abandoned)
                        on:change=move |ev| {
                            let v = event_target_checked(&ev);
                            characters
                                .update(|opt| {
                                    if let Some(list) = opt {
                                        if let Some(idx) = selected.get() {
                                            if let Some(c) = list.get_mut(idx) {
                                                c.is_abandoned = v;
                                            }
                                        }
                                    }
                                });
                        }
                    />
                    "Abandoned"
                </label>
            </div>
        </fieldset>
    }
}
