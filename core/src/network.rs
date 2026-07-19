use crate::settings::{load_settings_strict, mutate_settings_checked, NetworkSettings};
use url::Url;

const MAX_PROXY_URL_BYTES: usize = 2048;

pub fn get_proxy_settings() -> Result<NetworkSettings, String> {
    let settings = load_settings_strict().map_err(|error| error.to_string())?;
    let mut network = settings.network.unwrap_or_default();
    network.proxy_url = normalize_proxy_url(network.proxy_url)?;
    Ok(network)
}

pub fn set_proxy_url(proxy_url: Option<String>) -> Result<NetworkSettings, String> {
    let proxy_url = normalize_proxy_url(proxy_url)?;
    mutate_settings_checked(move |settings| {
        if proxy_url.is_none()
            && settings
                .network
                .as_ref()
                .map(|network| network.extra.is_empty())
                .unwrap_or(true)
        {
            settings.network = None;
            return Ok(NetworkSettings::default());
        }

        let network = settings
            .network
            .get_or_insert_with(NetworkSettings::default);
        network.proxy_url = proxy_url.clone();
        Ok(network.clone())
    })
    .map_err(|error| error.to_string())
}

pub fn configured_proxy_url() -> Result<Option<String>, String> {
    get_proxy_settings().map(|settings| settings.proxy_url)
}

pub fn build_ureq_agent(builder: ureq::AgentBuilder) -> Result<ureq::Agent, String> {
    let Some(proxy_url) = configured_proxy_url()? else {
        return Ok(builder.build());
    };
    let proxy =
        ureq::Proxy::new(&proxy_url).map_err(|_| "代理地址无效，请重新配置。".to_owned())?;
    Ok(builder.proxy(proxy).build())
}

fn normalize_proxy_url(proxy_url: Option<String>) -> Result<Option<String>, String> {
    let Some(proxy_url) = proxy_url else {
        return Ok(None);
    };
    let value = proxy_url.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > MAX_PROXY_URL_BYTES {
        return Err("代理地址过长。".into());
    }

    let mut url = Url::parse(value).map_err(|_| "请输入完整的代理地址。".to_owned())?;
    if !matches!(url.scheme(), "http" | "socks4" | "socks4a" | "socks5") {
        return Err("支持 HTTP、SOCKS4 和 SOCKS5 代理。".into());
    }
    if url.host_str().is_none() {
        return Err("代理地址缺少主机名。".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("代理地址不能包含用户名或密码。".into());
    }
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return Err("代理地址不能包含路径、查询参数或片段。".into());
    }

    url.set_path("");
    Ok(Some(url.as_str().trim_end_matches('/').to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testenv::TestHome;
    use serde_json::Value;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    #[test]
    fn proxy_url_is_normalized_and_can_be_disabled() {
        let home = TestHome::new("network-proxy-roundtrip");

        let saved = set_proxy_url(Some("  http://127.0.0.1:7890/  ".into())).unwrap();
        assert_eq!(saved.proxy_url.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(get_proxy_settings().unwrap(), saved);

        let disabled = set_proxy_url(Some("   ".into())).unwrap();
        assert_eq!(disabled.proxy_url, None);
        let value: Value = serde_json::from_str(
            &fs::read_to_string(home.home.join(".mux/settings.json")).unwrap(),
        )
        .unwrap();
        assert!(value.get("network").is_none());
    }

    #[test]
    fn supported_proxy_protocols_are_normalized() {
        for scheme in ["http", "socks4", "socks4a", "socks5"] {
            let value = format!("{scheme}://127.0.0.1:7890/");
            let expected = format!("{scheme}://127.0.0.1:7890");
            assert_eq!(
                normalize_proxy_url(Some(value)).unwrap().as_deref(),
                Some(expected.as_str())
            );
        }
    }

    #[test]
    fn disabling_proxy_preserves_future_network_fields() {
        let home = TestHome::new("network-proxy-passthrough");
        let path = home.home.join(".mux/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"network":{"proxy_url":"http://127.0.0.1:7890","future":{"keep":true}}}"#,
        )
        .unwrap();

        set_proxy_url(None).unwrap();

        let value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(value["network"].get("proxy_url").is_none());
        assert_eq!(value["network"]["future"]["keep"], true);
    }

    #[test]
    fn rejects_unsupported_or_sensitive_proxy_urls() {
        assert!(normalize_proxy_url(Some("127.0.0.1:7890".into())).is_err());
        assert!(normalize_proxy_url(Some("https://127.0.0.1:7890".into())).is_err());
        assert!(normalize_proxy_url(Some("socks5h://127.0.0.1:7890".into())).is_err());
        assert!(normalize_proxy_url(Some("http://user:secret@127.0.0.1:7890".into())).is_err());
        assert!(normalize_proxy_url(Some("http://127.0.0.1:7890/path".into())).is_err());
    }

    #[test]
    fn invalid_persisted_proxy_fails_closed() {
        let home = TestHome::new("network-proxy-invalid-persisted");
        let path = home.home.join(".mux/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"network":{"proxy_url":"https://127.0.0.1:7890"}}"#,
        )
        .unwrap();

        let error = build_ureq_agent(ureq::AgentBuilder::new()).unwrap_err();
        assert!(error.contains("SOCKS5"));
    }

    #[test]
    fn configured_proxy_is_used_by_ureq_agents() {
        let _home = TestHome::new("network-proxy-routing");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        set_proxy_url(Some(format!("http://{address}"))).unwrap();

        let proxy = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("GET http://example.invalid/proxy-check HTTP/1.1"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        });

        let response = build_ureq_agent(ureq::AgentBuilder::new())
            .unwrap()
            .get("http://example.invalid/proxy-check")
            .call()
            .unwrap()
            .into_string()
            .unwrap();

        assert_eq!(response, "ok");
        proxy.join().unwrap();
    }

    #[test]
    fn configured_socks5_proxy_is_used_by_ureq_agents() {
        let _home = TestHome::new("network-socks5-routing");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        set_proxy_url(Some(format!("socks5://{address}"))).unwrap();

        let proxy = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();

            let mut greeting = [0_u8; 3];
            stream.read_exact(&mut greeting).unwrap();
            assert_eq!(greeting, [5, 1, 0]);
            stream.write_all(&[5, 0]).unwrap();

            let mut request = [0_u8; 4];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(request, [5, 1, 0, 3]);
            let mut host_length = [0_u8; 1];
            stream.read_exact(&mut host_length).unwrap();
            let mut target = vec![0_u8; usize::from(host_length[0]) + 2];
            stream.read_exact(&mut target).unwrap();
            assert_eq!(&target[..target.len() - 2], b"example.invalid");
            assert_eq!(&target[target.len() - 2..], 80_u16.to_be_bytes());
            stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).unwrap();
            assert_origin_form_request_and_reply(&mut stream);
        });

        let response = build_ureq_agent(ureq::AgentBuilder::new())
            .unwrap()
            .get("http://example.invalid/proxy-check")
            .call()
            .unwrap()
            .into_string()
            .unwrap();

        assert_eq!(response, "ok");
        proxy.join().unwrap();
    }

    #[test]
    fn configured_socks4_proxy_is_used_by_ureq_agents() {
        let _home = TestHome::new("network-socks4-routing");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        set_proxy_url(Some(format!("socks4://{address}"))).unwrap();

        let proxy = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0_u8; 9];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request[..4], &[4, 1, 12, 138]);
            assert_eq!(&request[4..8], &[127, 0, 0, 1]);
            assert_eq!(request[8], 0);
            stream.write_all(&[0, 90, 12, 138, 127, 0, 0, 1]).unwrap();
            assert_origin_form_request_and_reply(&mut stream);
        });

        let response = build_ureq_agent(ureq::AgentBuilder::new())
            .unwrap()
            .get("http://127.0.0.1:3210/proxy-check")
            .call()
            .unwrap()
            .into_string()
            .unwrap();

        assert_eq!(response, "ok");
        proxy.join().unwrap();
    }

    fn assert_origin_form_request_and_reply(stream: &mut TcpStream) {
        let mut request = [0_u8; 2048];
        let read = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("GET /proxy-check HTTP/1.1"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    }

    #[test]
    fn mutation_refuses_to_replace_corrupt_settings() {
        let home = TestHome::new("network-proxy-corrupt-settings");
        let path = home.home.join(".mux/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{"network": ["#;
        fs::write(&path, original).unwrap();

        assert!(set_proxy_url(Some("http://127.0.0.1:7890".into())).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }
}
