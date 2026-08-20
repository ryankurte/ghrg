use tracing::{debug, trace};

#[derive(thiserror::Error, Debug)]
pub enum ReleaseError {
    #[error("API returned an error: {0}")]
    ApiError(String),
    #[error("Failed to fetch release info: {0}")]
    FetchError(#[from] reqwest::Error),
    #[error("Failed to parse release info: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Failed to read response text: {0}")]
    ResponseReadError(#[from] std::io::Error),
    #[error("Failed to parse response text as UTF-8: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
}

pub struct ReleaseClient {
    repo: String,
    client: reqwest::blocking::Client,
}

pub struct ReleaseInfo {
    json: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
}

impl ReleaseClient {
    /// Create a new [ReleaseClient] instance for the specified GitHub repository and optional Personal Access Token (PAT).
    pub fn new(repo: &str, token: Option<&str>) -> Self {
        // Setup the HTTP client

        let mut headers = reqwest::header::HeaderMap::new();

        headers.insert(reqwest::header::USER_AGENT, "ghrg/0.1.0".parse().unwrap());

        if let Some(token) = token.as_ref() {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("token {}", token).parse().unwrap(),
            );
        }

        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        Self { repo: repo.to_string(), client }
    }

    /// Generate the GitHub API URL for fetching release information for a given tag (or `latest`).
    pub fn release_url(&self, tag: Option<&str>) -> String {
        match tag {
            Some("latest") | None => {
                format!("https://api.github.com/repos/{}/releases/latest", self.repo)
            }
            Some(tag) => format!(
                "https://api.github.com/repos/{}/releases/tags/{}",
                self.repo, tag
            ),
        }
    }

    /// Fetch release information for a specific release tag (or `latest`)
    pub fn fetch_release_info(&self, tag: Option<&str>) -> Result<ReleaseInfo, ReleaseError> {
        // Generate the release info URL
        let url = self.release_url(tag);
        debug!("Fetching release info from URL: {}", url);

        // Fetch the release information
        let response = match self.client.get(&url).send() {
            Ok(r) => r,
            Err(e) => {
                debug!("Error fetching release info: {:?}", e);
                return Err(ReleaseError::FetchError(e));
            }
        };

        trace!("Received response: {:?}", response);
        
        if !response.status().is_success() {
            let response_text = response
                .text()
                .unwrap_or_else(|_| "Failed to read response text".to_string());
            debug!("Error response: {}", response_text);
            return Err(ReleaseError::ApiError(format!(
                "Failed to fetch release info: {}",
                response_text
            )));
        }

        // Parse the response as JSON
        let json = response.json()?;

        Ok(ReleaseInfo { json })
    }

    /// Download a release asset from the specified URL and save it to the given file path.
    pub fn download_asset(
        &self,
        asset: &ReleaseAsset,
        output_path: &std::path::Path,
    ) -> Result<(), ReleaseError> {
        debug!("Downloading asset '{}' from URL: {}", asset.name, asset.download_url);

        let mut response = self.client.get(&asset.download_url)
            .header("Accept", "application/octet-stream").send()?;

        if !response.status().is_success() {
            let response_text = response
                .text()
                .unwrap_or_else(|_| "Failed to read response text".to_string());
            debug!("Error response: {}", response_text);
            return Err(ReleaseError::ApiError(format!(
                "Failed to download asset: {}",
                response_text
            )));
        }

        debug!("Saving asset to path: {:?}", output_path);

        let mut file = std::fs::File::create(output_path)?;
        std::io::copy(&mut response, &mut file)?;

        Ok(())
    }
}

impl ReleaseInfo {
    /// Fetch the release tag.
    pub fn tag(&self) -> Option<String> {
        self.json
            .get("tag_name")
            .and_then(|tag| tag.as_str().map(|s| s.to_string()))
    }

    /// Fetch a list of release assets.
    pub fn assets(&self) -> Vec<ReleaseAsset> {
        self.json
            .get("assets")
            .and_then(|assets| assets.as_array())
            .map(|assets| {
                assets
                    .iter()
                    .filter_map(|asset| {
                        let name = asset.get("name")?.as_str()?.to_string();
                        let download_url = asset.get("url")?.as_str()?.to_string();
                        Some(ReleaseAsset { name, download_url })
                    })
                    .collect()
            })
            .unwrap_or_else(Vec::new)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test_log::test]
    fn test_release_url() {
        let client = ReleaseClient::new("owner/repo", None);
        assert_eq!(
            client.release_url(Some("latest")),
            "https://api.github.com/repos/owner/repo/releases/latest"
        );
        assert_eq!(
            client.release_url(Some("v1.0.0")),
            "https://api.github.com/repos/owner/repo/releases/tags/v1.0.0"
        );
        assert_eq!(
            client.release_url(None),
            "https://api.github.com/repos/owner/repo/releases/latest"
        );
    }

    #[test_log::test]
    fn test_fetch_release_info_latest() {
        let client = ReleaseClient::new("ryankurte/rust-ptouch", None);
        let result = client
            .fetch_release_info(Some("latest"))
            .expect("Failed to fetch release info");
        assert!(result.json.is_object());
    }

    #[test_log::test]
    fn test_fetch_release_info_tag() {
        let client = ReleaseClient::new("ryankurte/rust-ptouch", None);
        let result = client
            .fetch_release_info(Some("v0.2.2"))
            .expect("Failed to fetch release info");
        assert!(result.json.is_object());
    }

    #[test_log::test]
    fn parse_release_info_example() {
        let release_info: serde_json::Value =
            serde_json::from_str(RELEASE_INFO).expect("Failed to parse release info");
        assert_eq!(release_info["tag_name"], "v0.2.2");
        assert_eq!(release_info["assets"].as_array().unwrap().len(), 4);
    }

    const RELEASE_INFO: &str = r#"
{
  "url": "https://api.github.com/repos/ryankurte/rust-ptouch/releases/88989464",
  "assets_url": "https://api.github.com/repos/ryankurte/rust-ptouch/releases/88989464/assets",
  "upload_url": "https://uploads.github.com/repos/ryankurte/rust-ptouch/releases/88989464/assets{?name,label}",
  "html_url": "https://github.com/ryankurte/rust-ptouch/releases/tag/v0.2.2",
  "id": 88989464,
  "author": {
    "login": "github-actions[bot]",
    "id": 41898282,
    "node_id": "MDM6Qm90NDE4OTgyODI=",
    "avatar_url": "https://avatars.githubusercontent.com/in/15368?v=4",
    "gravatar_id": "",
    "url": "https://api.github.com/users/github-actions%5Bbot%5D",
    "html_url": "https://github.com/apps/github-actions",
    "followers_url": "https://api.github.com/users/github-actions%5Bbot%5D/followers",
    "following_url": "https://api.github.com/users/github-actions%5Bbot%5D/following{/other_user}",
    "gists_url": "https://api.github.com/users/github-actions%5Bbot%5D/gists{/gist_id}",
    "starred_url": "https://api.github.com/users/github-actions%5Bbot%5D/starred{/owner}{/repo}",
    "subscriptions_url": "https://api.github.com/users/github-actions%5Bbot%5D/subscriptions",
    "organizations_url": "https://api.github.com/users/github-actions%5Bbot%5D/orgs",
    "repos_url": "https://api.github.com/users/github-actions%5Bbot%5D/repos",
    "events_url": "https://api.github.com/users/github-actions%5Bbot%5D/events{/privacy}",
    "received_events_url": "https://api.github.com/users/github-actions%5Bbot%5D/received_events",
    "type": "Bot",
    "user_view_type": "public",
    "site_admin": false
  },
  "node_id": "RE_kwDOE9QJuM4FTd8Y",
  "tag_name": "v0.2.2",
  "target_commitish": "8cf88961a2b9b24fab245b51e8a0457501506dcf",
  "name": "Release v0.2.2",
  "draft": false,
  "immutable": false,
  "prerelease": false,
  "created_at": "2023-01-14T21:32:02Z",
  "updated_at": "2023-01-14T21:37:48Z",
  "published_at": "2023-01-14T21:32:15Z",
  "assets": [
    {
      "url": "https://api.github.com/repos/ryankurte/rust-ptouch/releases/assets/91706153",
      "id": 91706153,
      "node_id": "RA_kwDOE9QJuM4Fd1Mp",
      "name": "ptouch-util-armv7-unknown-linux-gnueabihf.tgz",
      "label": "",
      "uploader": {
        "login": "github-actions[bot]",
        "id": 41898282,
        "node_id": "MDM6Qm90NDE4OTgyODI=",
        "avatar_url": "https://avatars.githubusercontent.com/in/15368?v=4",
        "gravatar_id": "",
        "url": "https://api.github.com/users/github-actions%5Bbot%5D",
        "html_url": "https://github.com/apps/github-actions",
        "followers_url": "https://api.github.com/users/github-actions%5Bbot%5D/followers",
        "following_url": "https://api.github.com/users/github-actions%5Bbot%5D/following{/other_user}",
        "gists_url": "https://api.github.com/users/github-actions%5Bbot%5D/gists{/gist_id}",
        "starred_url": "https://api.github.com/users/github-actions%5Bbot%5D/starred{/owner}{/repo}",
        "subscriptions_url": "https://api.github.com/users/github-actions%5Bbot%5D/subscriptions",
        "organizations_url": "https://api.github.com/users/github-actions%5Bbot%5D/orgs",
        "repos_url": "https://api.github.com/users/github-actions%5Bbot%5D/repos",
        "events_url": "https://api.github.com/users/github-actions%5Bbot%5D/events{/privacy}",
        "received_events_url": "https://api.github.com/users/github-actions%5Bbot%5D/received_events",
        "type": "Bot",
        "user_view_type": "public",
        "site_admin": false
      },
      "content_type": "binary/octet-stream",
      "state": "uploaded",
      "size": 2259764,
      "digest": null,
      "download_count": 10,
      "created_at": "2023-01-14T21:35:51Z",
      "updated_at": "2023-01-14T21:35:52Z",
      "browser_download_url": "https://github.com/ryankurte/rust-ptouch/releases/download/v0.2.2/ptouch-util-armv7-unknown-linux-gnueabihf.tgz"
    },
    {
      "url": "https://api.github.com/repos/ryankurte/rust-ptouch/releases/assets/91706135",
      "id": 91706135,
      "node_id": "RA_kwDOE9QJuM4Fd1MX",
      "name": "ptouch-util-x86_64-apple-darwin.tgz",
      "label": "",
      "uploader": {
        "login": "github-actions[bot]",
        "id": 41898282,
        "node_id": "MDM6Qm90NDE4OTgyODI=",
        "avatar_url": "https://avatars.githubusercontent.com/in/15368?v=4",
        "gravatar_id": "",
        "url": "https://api.github.com/users/github-actions%5Bbot%5D",
        "html_url": "https://github.com/apps/github-actions",
        "followers_url": "https://api.github.com/users/github-actions%5Bbot%5D/followers",
        "following_url": "https://api.github.com/users/github-actions%5Bbot%5D/following{/other_user}",
        "gists_url": "https://api.github.com/users/github-actions%5Bbot%5D/gists{/gist_id}",
        "starred_url": "https://api.github.com/users/github-actions%5Bbot%5D/starred{/owner}{/repo}",
        "subscriptions_url": "https://api.github.com/users/github-actions%5Bbot%5D/subscriptions",
        "organizations_url": "https://api.github.com/users/github-actions%5Bbot%5D/orgs",
        "repos_url": "https://api.github.com/users/github-actions%5Bbot%5D/repos",
        "events_url": "https://api.github.com/users/github-actions%5Bbot%5D/events{/privacy}",
        "received_events_url": "https://api.github.com/users/github-actions%5Bbot%5D/received_events",
        "type": "Bot",
        "user_view_type": "public",
        "site_admin": false
      },
      "content_type": "binary/octet-stream",
      "state": "uploaded",
      "size": 1337619,
      "digest": null,
      "download_count": 22,
      "created_at": "2023-01-14T21:35:38Z",
      "updated_at": "2023-01-14T21:35:38Z",
      "browser_download_url": "https://github.com/ryankurte/rust-ptouch/releases/download/v0.2.2/ptouch-util-x86_64-apple-darwin.tgz"
    },
    {
      "url": "https://api.github.com/repos/ryankurte/rust-ptouch/releases/assets/91706348",
      "id": 91706348,
      "node_id": "RA_kwDOE9QJuM4Fd1Ps",
      "name": "ptouch-util-x86_64-pc-windows-msvc.tgz",
      "label": "",
      "uploader": {
        "login": "github-actions[bot]",
        "id": 41898282,
        "node_id": "MDM6Qm90NDE4OTgyODI=",
        "avatar_url": "https://avatars.githubusercontent.com/in/15368?v=4",
        "gravatar_id": "",
        "url": "https://api.github.com/users/github-actions%5Bbot%5D",
        "html_url": "https://github.com/apps/github-actions",
        "followers_url": "https://api.github.com/users/github-actions%5Bbot%5D/followers",
        "following_url": "https://api.github.com/users/github-actions%5Bbot%5D/following{/other_user}",
        "gists_url": "https://api.github.com/users/github-actions%5Bbot%5D/gists{/gist_id}",
        "starred_url": "https://api.github.com/users/github-actions%5Bbot%5D/starred{/owner}{/repo}",
        "subscriptions_url": "https://api.github.com/users/github-actions%5Bbot%5D/subscriptions",
        "organizations_url": "https://api.github.com/users/github-actions%5Bbot%5D/orgs",
        "repos_url": "https://api.github.com/users/github-actions%5Bbot%5D/repos",
        "events_url": "https://api.github.com/users/github-actions%5Bbot%5D/events{/privacy}",
        "received_events_url": "https://api.github.com/users/github-actions%5Bbot%5D/received_events",
        "type": "Bot",
        "user_view_type": "public",
        "site_admin": false
      },
      "content_type": "binary/octet-stream",
      "state": "uploaded",
      "size": 1110177,
      "digest": null,
      "download_count": 35,
      "created_at": "2023-01-14T21:37:48Z",
      "updated_at": "2023-01-14T21:37:48Z",
      "browser_download_url": "https://github.com/ryankurte/rust-ptouch/releases/download/v0.2.2/ptouch-util-x86_64-pc-windows-msvc.tgz"
    },
    {
      "url": "https://api.github.com/repos/ryankurte/rust-ptouch/releases/assets/91706109",
      "id": 91706109,
      "node_id": "RA_kwDOE9QJuM4Fd1L9",
      "name": "ptouch-util-x86_64-unknown-linux-gnu.tgz",
      "label": "",
      "uploader": {
        "login": "github-actions[bot]",
        "id": 41898282,
        "node_id": "MDM6Qm90NDE4OTgyODI=",
        "avatar_url": "https://avatars.githubusercontent.com/in/15368?v=4",
        "gravatar_id": "",
        "url": "https://api.github.com/users/github-actions%5Bbot%5D",
        "html_url": "https://github.com/apps/github-actions",
        "followers_url": "https://api.github.com/users/github-actions%5Bbot%5D/followers",
        "following_url": "https://api.github.com/users/github-actions%5Bbot%5D/following{/other_user}",
        "gists_url": "https://api.github.com/users/github-actions%5Bbot%5D/gists{/gist_id}",
        "starred_url": "https://api.github.com/users/github-actions%5Bbot%5D/starred{/owner}{/repo}",
        "subscriptions_url": "https://api.github.com/users/github-actions%5Bbot%5D/subscriptions",
        "organizations_url": "https://api.github.com/users/github-actions%5Bbot%5D/orgs",
        "repos_url": "https://api.github.com/users/github-actions%5Bbot%5D/repos",
        "events_url": "https://api.github.com/users/github-actions%5Bbot%5D/events{/privacy}",
        "received_events_url": "https://api.github.com/users/github-actions%5Bbot%5D/received_events",
        "type": "Bot",
        "user_view_type": "public",
        "site_admin": false
      },
      "content_type": "binary/octet-stream",
      "state": "uploaded",
      "size": 2250180,
      "digest": null,
      "download_count": 38,
      "created_at": "2023-01-14T21:35:17Z",
      "updated_at": "2023-01-14T21:35:18Z",
      "browser_download_url": "https://github.com/ryankurte/rust-ptouch/releases/download/v0.2.2/ptouch-util-x86_64-unknown-linux-gnu.tgz"
    }
  ],
  "tarball_url": "https://api.github.com/repos/ryankurte/rust-ptouch/tarball/v0.2.2",
  "zipball_url": "https://api.github.com/repos/ryankurte/rust-ptouch/zipball/v0.2.2",
  "body": "Release refs/tags/v0.2.2"
}"#;
}
