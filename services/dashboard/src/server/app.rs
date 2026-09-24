use super::{
    docker::{self, DockerClient},
    logo,
};
use home_ui::Service;
use leptos::prelude::LeptosOptions;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct App {
    docker: DockerClient,
    pub leptos_options: LeptosOptions,
    extras: Vec<Service>,
    logo_urls: RwLock<HashMap<String, String>>,
    pub logo_client: reqwest::Client,
}

impl App {
    pub fn new(
        socket: String,
        leptos_options: LeptosOptions,
        extras: Vec<Service>,
    ) -> Result<Self, reqwest::Error> {
        Ok(Self {
            docker: DockerClient::new(socket)?,
            leptos_options,
            extras: extras
                .into_iter()
                .filter(|service| {
                    if docker::valid_service(service) {
                        true
                    } else {
                        eprintln!(
                            "Warning: ignoring invalid DASHBOARD_LINKS service: name={:?}, url={:?}",
                            service.name, service.url
                        );
                        false
                    }
                })
                .collect(),
            logo_urls: RwLock::new(HashMap::new()),
            logo_client: logo::client()?,
        })
    }

    pub async fn services(&self) -> Vec<Service> {
        let mut services = match self.docker.discover().await {
            Ok(services) => services,
            Err(error) => {
                eprintln!("Docker discovery failed: {error}");
                Vec::new()
            }
        };
        services.extend_from_slice(&self.extras);

        // This allows us to not have to handroll a cache or call services() multiple times
        // when a session requests logos for the full page. Instead `logo_urls` will point
        // directly to the last known service url for a given service key
        *self.logo_urls.write().await = services
            .iter()
            .map(|service| (service.logo_path(), service.url.clone()))
            .collect();

        services
    }

    pub async fn logo_url(&self, key: &str) -> Option<String> {
        self.logo_urls
            .read()
            .await
            .get(&format!("/logo/{key}"))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn logo_lookup_uses_services_from_last_page_render() {
        let service = Service {
            name: "Example".into(),
            url: "https://example.com".into(),
            group: "Services".into(),
            description: String::new(),
        };
        let app = App::new(
            "/tmp/dashboard-test-missing-docker.sock".into(),
            LeptosOptions::builder().output_name("dashboard").build(),
            vec![service.clone()],
        )
        .unwrap();
        let key = service.logo_path();
        let key = key.strip_prefix("/logo/").unwrap();

        assert!(app.logo_url(key).await.is_none());
        app.services().await;
        assert_eq!(
            app.logo_url(key).await.as_deref(),
            Some(service.url.as_str())
        );
        assert!(app.logo_url("unknown").await.is_none());
    }
}
