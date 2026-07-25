use crate::data::BUNDLE;
use crate::icon::Icon;
use crate::profile::ProfileSave;
use leptos::prelude::*;

#[component]
pub fn ProfileFieldsPanel(profile: RwSignal<Option<Vec<ProfileSave>>>, selected: RwSignal<Option<usize>>) -> impl IntoView {
    view! {
        <fieldset class="currency-fields">
            <legend>"Currencies"</legend>
            <div class="field-grid">
                {BUNDLE
                    .currencies
                    .iter()
                    .map(|currency| {
                        let meta_row = currency.name.clone();
                        let meta_row_set = meta_row.clone();
                        // Per-currency accent color, keyed by RowName in CSS
                        // (e.g. .currency-Exotic_Red).
                        let class = format!("currency-field currency-{}", currency.name);
                        view! {
                            <label class=class title=currency.description.clone()>
                                <Icon file=currency.icon_file.clone() size=28 />
                                <span class="currency-name">{currency.display_name.clone()}</span>
                                <input
                                    type="number"
                                    min="0"
                                    max="999999"
                                    prop:value=move || {
                                        profile
                                            .with(|opt| {
                                                opt.as_ref()
                                                    .zip(selected.get())
                                                    .and_then(|(list, i)| list.get(i))
                                                    .map(|p| p.currency(&meta_row))
                                                    .unwrap_or(0)
                                            })
                                    }
                                    on:input=move |ev| {
                                        if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                            profile
                                                .update(|opt| {
                                                    if let Some(list) = opt {
                                                        if let Some(idx) = selected.get() {
                                                            if let Some(p) = list.get_mut(idx) {
                                                                p.set_currency(&meta_row_set, v);
                                                            }
                                                        }
                                                    }
                                                });
                                        }
                                    }
                                />
                            </label>
                        }
                    })
                    .collect_view()}
            </div>
        </fieldset>
    }
}
