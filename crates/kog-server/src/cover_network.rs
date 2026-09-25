//! Bounded HTTPS cover requests for the Web artwork endpoint.

use std::io::Read;
use std::time::Duration;

use url::Url;

fn allowed(url: &Url) -> bool {
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.port(), None | Some(443))
    {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    matches!(
        host,
        "api.deezer.com"
            | "cdn-images.dzcdn.net"
            | "itunes.apple.com"
            | "musicbrainz.org"
            | "coverartarchive.org"
            | "archive.org"
            | "duckduckgo.com"
    ) || host.ends_with(".mzstatic.com")
        || host.ends_with(".archive.org")
}

pub fn fetch(url: &str, max_bytes: u32) -> Option<Vec<u8>> {
    if max_bytes > 16 * 1024 * 1024 {
        return None;
    }
    let mut current = Url::parse(url).ok()?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .max_redirects(0)
        .build()
        .into();
    for _ in 0..=5 {
        if !allowed(&current) {
            return None;
        }
        let mut response = None;
        for attempt in 0..2 {
            match agent
                .get(current.as_str())
                .header(
                    "User-Agent",
                    concat!("Kog/", env!("CARGO_PKG_VERSION"), " (web cover art)"),
                )
                .call()
            {
                Ok(found) => {
                    response = Some(found);
                    break;
                }
                Err(ureq::Error::StatusCode(429 | 500 | 502 | 503 | 504)) if attempt == 0 => {
                    std::thread::sleep(Duration::from_millis(1200));
                }
                Err(_) => return None,
            }
        }
        let response = response?;
        if response.status().is_redirection() {
            let location = response.headers().get("location")?.to_str().ok()?;
            current = current.join(location).ok()?;
            continue;
        }
        if !response.status().is_success() {
            return None;
        }
        let mut bytes = Vec::new();
        response
            .into_body()
            .as_reader()
            .take(u64::from(max_bytes) + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        return (bytes.len() <= max_bytes as usize).then_some(bytes);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_fetch_allowlist_rejects_credentials_http_and_unknown_hosts() {
        assert!(allowed(
            &Url::parse("https://cdn-images.dzcdn.net/cover.jpg").unwrap()
        ));
        assert!(allowed(
            &Url::parse("https://is1.mzstatic.com/a.jpg").unwrap()
        ));
        assert!(!allowed(&Url::parse("http://api.deezer.com/").unwrap()));
        assert!(!allowed(
            &Url::parse("https://api.deezer.com.evil.test/").unwrap()
        ));
        assert!(!allowed(
            &Url::parse("https://user:pass@archive.org/").unwrap()
        ));
        assert!(!allowed(&Url::parse("https://archive.org:8443/").unwrap()));
    }
}
