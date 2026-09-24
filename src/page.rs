use crate::Service;

pub fn render_page(services: &[Service], options: leptos::prelude::LeptosOptions) -> String {
    use crate::search::QuickSearch;
    use leptos::prelude::*;
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<&str, Vec<&Service>> = BTreeMap::new();
    for service in services {
        groups.entry(&service.group).or_default().push(service);
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    let summary = format!("{} services · {} groups", services.len(), groups.len());
    groups.sort_by_key(|(group, _)| match *group {
        "Services" => 0,
        "Media" => 1,
        "Infrastructure" => 2,
        _ => 3,
    });
    let sections = groups
        .into_iter()
        .map(|(group, mut services)| {
            services.sort_by_key(|service| service.name.to_lowercase());
            let count = services.len();
            let rows = services
            .into_iter()
            .map(|service| {
                let initial = service
                    .name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let address = service
                    .url
                    .split_once("://")
                    .map(|(_, rest)| rest.split('/').next().unwrap_or(rest))
                    .unwrap_or(&service.url);

                view! {
                    <li>
                        <a class="service-row" href=service.url.clone()>
                            <span class="icon">
                                <span>{initial}</span>
                                <img class="favicon" src=service.logo_path() alt="" loading="lazy"/>
                            </span>
                            <strong>{service.name.clone()}</strong>
                            <span class="description">{service.description.clone()}</span>
                            <span class="address">{address.to_string()}</span>
                            <span class="arrow" aria-hidden="true">"↗"</span>
                        </a>
                    </li>
                }
            })
            .collect_view();

            view! {
                <section>
                    <div class="section-heading">
                        <h2>{group.to_string()}</h2>
                        <span>{count.to_string()}</span>
                    </div>
                    <ul class="service-list">{rows}</ul>
                </section>
            }
        })
        .collect_view();

    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width,initial-scale=1"/>
                <meta name="color-scheme" content="light"/>
                <title>"Service directory"</title>
                <link rel="icon" type="image/svg+xml" href="/favicon.svg"/>
                <link rel="stylesheet" href="/pkg/webdash-rs.css"/>
                <HydrationScripts options islands=true/>
            </head>
            <body>
                <main>
                    <header class="hero">
                        <div>
                            <h1>"Service directory"</h1>
                            <p class="intro">{summary}</p>
                        </div>
                        <QuickSearch services=services.to_vec()/>
                    </header>
                    {services.is_empty().then(|| view! {
                        <p class="empty">
                            "No services yet. Add dashboard labels to a running container."
                        </p>
                    })}
                    {sections}
                </main>
            </body>
        </html>
    }
    .to_html()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_labels_in_server_render() {
        let page = render_page(
            &[Service {
                name: "<script>".into(),
                url: "https://example.com".into(),
                group: "A&B".into(),
                description: "\"quoted\"".into(),
            }],
            leptos::prelude::get_configuration(None)
                .unwrap()
                .leptos_options,
        );
        assert!(page.contains("&lt;script&gt;"));
        assert!(page.contains("A&amp;B"));
        assert!(!page.contains("<script>"));
    }
}
