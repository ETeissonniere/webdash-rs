use crate::Service;
use leptos::prelude::*;
use std::sync::Arc;
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Element, HtmlInputElement};
use web_sys::{HtmlImageElement, KeyboardEvent};

/// Scores an in-order character match, favoring word starts and consecutive letters.
fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let query: Vec<char> = query
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let candidate: Vec<char> = candidate.to_lowercase().chars().collect();
    let mut position = 0;
    let mut score = 0;
    let mut previous = None;
    for wanted in query {
        let index = (position..candidate.len()).find(|&index| candidate[index] == wanted)?;
        score += 10;
        if index == 0 || !candidate[index - 1].is_alphanumeric() {
            score += 8;
        }
        if previous.is_some_and(|last| index == last + 1) {
            score += 5;
        }
        score -= index as i32 - position as i32;
        previous = Some(index);
        position = index + 1;
    }
    Some(score)
}

fn open_search(open: RwSignal<bool>, query: RwSignal<String>, selected: RwSignal<usize>) {
    open.set(true);
    query.set(String::new());
    selected.set(0);
}

#[island]
pub fn QuickSearch(services: Vec<Service>) -> impl IntoView {
    let open = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let selected = RwSignal::new(0usize);
    let services = Arc::new(services);

    #[cfg(target_arch = "wasm32")]
    {
        browser::hide_missing_favicons();
        browser::install_shortcuts(open, query, selected);
        Effect::new(move || {
            if open.get() {
                if let Some(input) = web_sys::window()
                    .unwrap()
                    .document()
                    .unwrap()
                    .get_element_by_id("search-input")
                    .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
                {
                    input.focus().ok();
                }
            }
        });
    }

    let ranked = Memo::new({
        let services = services.clone();
        move |_: Option<&Vec<Service>>| {
            let text = query.get();
            let mut ranked: Vec<_> = services
                .iter()
                .filter_map(|service| {
                    fuzzy_score(&text, &format!("{} {}", service.name, service.group))
                        .map(|score| (score, service.clone()))
                })
                .collect();
            ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
            ranked
                .into_iter()
                .take(12)
                .map(|(_, service)| service)
                .collect::<Vec<_>>()
        }
    });
    let on_key = move |event: KeyboardEvent| {
        let count = ranked.get().len();
        match event.key().as_str() {
            "ArrowDown" if count > 0 => {
                event.prevent_default();
                selected.update(|index| *index = (*index + 1) % count);
            }
            "ArrowUp" if count > 0 => {
                event.prevent_default();
                selected.update(|index| *index = (*index + count - 1) % count);
            }
            "Enter" if count > 0 => {
                event.prevent_default();
                let url = &ranked.get()[selected.get().min(count - 1)].url;
                if let Some(window) = web_sys::window() {
                    window.location().set_href(url).ok();
                }
            }
            "Escape" => open.set(false),
            _ => {}
        }
    };
    view! {
        <button
            id="open-search"
            class="search-trigger"
            type="button"
            on:click=move |_| open_search(open, query, selected)
        >
            <span class="search-symbol" aria-hidden="true">"⌕"</span>
            "Search services"
            <kbd>"⌘ K"</kbd>
        </button>
        <div
            class="search-overlay"
            style:display=move || if open.get() { "grid" } else { "none" }
            on:click=move |_| open.set(false)
        >
            <div
                class="search-panel"
                role="dialog"
                aria-modal="true"
                aria-label="Search services"
                on:click=move |event| event.stop_propagation()
            >
                <input
                    id="search-input"
                    type="search"
                    placeholder="Search services…"
                    aria-label="Search services"
                    autocomplete="off"
                    prop:value=move || query.get()
                    on:input:target=move |event| {
                        query.set(event.target().value());
                        selected.set(0);
                    }
                    on:keydown=on_key
                />
                <div id="search-results">
                    {move || {
                        let rows = ranked.get();
                        if rows.is_empty() {
                            return view! {
                                <p class="no-results">"No matching services"</p>
                            }
                            .into_any();
                        }

                        rows.into_iter()
                            .enumerate()
                            .map(|(index, service)| {
                                let initial = service
                                    .name
                                    .chars()
                                    .next()
                                    .unwrap_or('?')
                                    .to_uppercase()
                                    .to_string();
                                view! {
                                    <a
                                        class=move || if selected.get() == index {
                                            "search-result selected"
                                        } else {
                                            "search-result"
                                        }
                                        href=service.url.clone()
                                    >
                                        <span class="search-icon">
                                            <span>{initial}</span>
                                            <img
                                                src=service.logo_path()
                                                alt=""
                                                on:error=move |event| {
                                                    if let Some(image) = event
                                                        .target()
                                                        .and_then(|target| target.dyn_into::<HtmlImageElement>().ok())
                                                    {
                                                        image.set_hidden(true);
                                                    }
                                                }
                                            />
                                        </span>
                                        <strong>{service.name}</strong>
                                        <small>{service.group}</small>
                                    </a>
                                }
                            })
                            .collect_view()
                            .into_any()
                    }}
                </div>
                <p class="search-hint">"↑ ↓ to move · Enter to open · Esc to close"</p>
            </div>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use wasm_bindgen::closure::Closure;

    pub fn hide_missing_favicons() {
        let images = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .query_selector_all(".favicon")
            .unwrap();
        for index in 0..images.length() {
            let Some(image) = images
                .item(index)
                .and_then(|node| node.dyn_into::<HtmlImageElement>().ok())
            else {
                continue;
            };
            if image.complete() && image.natural_width() == 0 {
                image.set_hidden(true);
            }
            let on_error = {
                let image = image.clone();
                Closure::<dyn FnMut()>::new(move || image.set_hidden(true))
            };
            image
                .add_event_listener_with_callback("error", on_error.as_ref().unchecked_ref())
                .ok();
            on_error.forget();
        }
    }

    pub fn install_shortcuts(
        open: RwSignal<bool>,
        query: RwSignal<String>,
        selected: RwSignal<usize>,
    ) {
        let shortcut = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            let command_k =
                (event.meta_key() || event.ctrl_key()) && event.key().eq_ignore_ascii_case("k");
            let slash = event.key() == "/"
                && !event.alt_key()
                && !event.meta_key()
                && !event.ctrl_key()
                && !event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                    .is_some_and(|element| {
                        element
                            .matches("input, textarea, [contenteditable]")
                            .unwrap_or(false)
                    });
            if command_k || slash {
                event.prevent_default();
                open_search(open, query, selected);
            } else if event.key() == "Escape" {
                open.set(false);
            }
        });
        web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .add_event_listener_with_callback("keydown", shortcut.as_ref().unchecked_ref())
            .unwrap();
        shortcut.forget();
    }
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn ranks_contiguous_matches_first() {
        assert!(fuzzy_score("jf", "Jellyfin").is_some());
        assert!(fuzzy_score("son", "Sonarr") > fuzzy_score("son", "Service One"));
        assert_eq!(fuzzy_score("xyz", "Sonarr"), None);
    }
}
