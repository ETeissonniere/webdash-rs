use home_ui::{default_group, Service};
use reqwest::Client;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Container {
    labels: BTreeMap<String, String>,
}

pub struct DockerClient {
    client: Client,
}

impl DockerClient {
    pub fn new(socket: impl AsRef<Path>) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .unix_socket(socket.as_ref())
                .timeout(Duration::from_secs(3))
                .build()?,
        })
    }

    pub async fn discover(&self) -> Result<Vec<Service>, reqwest::Error> {
        let services = self
            .client
            .get("http://docker/containers/json?all=0")
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Container>>()
            .await?
            .into_iter()
            .filter_map(|container| {
                let labels = container.labels;
                Some(Service {
                    name: labels.get("dashboard.name")?.clone(),
                    url: labels.get("dashboard.url")?.clone(),
                    group: labels
                        .get("dashboard.group")
                        .cloned()
                        .unwrap_or_else(default_group),
                    description: labels
                        .get("dashboard.description")
                        .cloned()
                        .unwrap_or_default(),
                })
            })
            .filter(valid_service)
            .collect();
        Ok(services)
    }
}

pub fn valid_service(service: &Service) -> bool {
    !service.name.trim().is_empty()
        && Url::parse(&service.url)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    #[test]
    fn rejects_non_web_urls() {
        let mut service = Service {
            name: "Service".into(),
            url: "javascript:alert(1)".into(),
            group: default_group(),
            description: String::new(),
        };

        assert!(!valid_service(&service));
        service.url = "http://example.com:invalid".into();
        assert!(!valid_service(&service));
        service.url = "https://example.com".into();
        assert!(valid_service(&service));
    }

    #[tokio::test]
    async fn discovers_services_over_docker_socket() {
        let path = PathBuf::from(format!("/tmp/dashboard-docker-{}.sock", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 512];
            let size = stream.read(&mut request).unwrap();
            assert!(request[..size].starts_with(b"GET /containers/json?all=0 HTTP/1.1"));

            let body = r#"[{"Labels":{"dashboard.name":"Jellyfin","dashboard.url":"https://jellyfin.example.com"}},{"Labels":{"other":"ignore"}}]"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });

        let client = DockerClient::new(&path).unwrap();
        let services = client.discover().await.unwrap();
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "Jellyfin");
    }
}
