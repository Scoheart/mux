use super::{full_request_url, provider_credential_subject, read_credential};
use crate::domain::types::{ModelProtocol, ModelProviderConfig};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;
use std::time::Duration;
use url::Url;
use zeroize::Zeroizing;

const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MODELS: usize = 2_000;
const MAX_PAGES: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderModelSummary {
    pub id: String,
    pub name: Option<String>,
    pub context_length: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiscoveryAdapter {
    OpenAi,
    Anthropic,
    Gemini,
    Cohere,
    Fireworks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialPolicy {
    Required,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiscoverySpec {
    adapter: DiscoveryAdapter,
    credential: CredentialPolicy,
}

#[derive(Debug, PartialEq, Eq)]
struct DecodedPage {
    models: Vec<ProviderModelSummary>,
    next_token: Option<String>,
}

pub(super) fn model_discovery_supported(_provider_type: &str) -> bool {
    true
}

pub fn discover_provider_models(provider_id: &str) -> Result<Vec<ProviderModelSummary>, String> {
    let settings = crate::settings::load_settings_strict().map_err(|error| error.to_string())?;
    let provider = settings
        .model_providers
        .as_ref()
        .and_then(|providers| providers.get(provider_id))
        .ok_or_else(|| {
            format!("model_provider_not_found: Provider '{provider_id}' does not exist")
        })?;
    let spec = discovery_spec_for_provider(provider)?;

    let credential = read_credential(&provider_credential_subject(provider_id))
        .map(String::from_utf8)
        .transpose()
        .map_err(|_| {
            format!(
                "model_provider_credential_invalid: Provider '{provider_id}' has a non-UTF-8 API Key"
            )
        })?
        .map(Zeroizing::new);
    if spec.credential == CredentialPolicy::Required && credential.is_none() {
        return Err(format!(
            "model_provider_credential_missing: Save an API Key for Provider '{provider_id}' before loading its model catalog"
        ));
    }

    let agent = discovery_agent()?;
    let mut url = discovery_url(provider, spec.adapter)?;
    let mut models = Vec::new();
    for page_index in 0..MAX_PAGES {
        let page = fetch_page(
            &agent,
            &url,
            spec.adapter,
            credential.as_ref().map(|value| value.as_str()),
        )?;
        merge_models(&mut models, page.models)?;
        let Some(next_token) = page.next_token else {
            return Ok(models);
        };
        if page_index + 1 == MAX_PAGES {
            return Err(format!(
                "model_discovery_too_many_pages: Provider catalog exceeded the {MAX_PAGES}-page safety limit"
            ));
        }
        url = next_page_url(url, spec.adapter, &next_token)?;
    }
    Ok(models)
}

fn reviewed_discovery_spec(provider_type: &str) -> Option<DiscoverySpec> {
    let adapter = match provider_type {
        "openrouter"
        | "openai"
        | "xai"
        | "mistral"
        | "deepseek"
        | "groq"
        | "alibaba"
        | "alibaba-coding-plan-cn"
        | "alibaba-coding-plan"
        | "alibaba-token-plan-cn"
        | "alibaba-token-plan"
        | "xiaomi"
        | "xiaomi-token-plan-cn"
        | "xiaomi-token-plan-sgp"
        | "xiaomi-token-plan-ams"
        | "moonshotai"
        | "kimi-for-coding"
        | "zai"
        | "zai-coding-plan"
        | "zhipuai-coding-plan"
        | "minimax-coding-plan"
        | "minimax-cn-coding-plan"
        | "stepfun-step-plan"
        | "stepfun-ai-step-plan"
        | "tencent-coding-plan"
        | "tencent-token-plan"
        | "tencent-token-plan-global"
        | "nvidia"
        | "cerebras"
        | "siliconflow"
        | "together"
        | "poe"
        | "huggingface"
        | "novita-ai"
        | "qiniu-ai"
        | "digitalocean"
        | "modelscope"
        | "scaleway"
        | "nebius"
        | "requesty"
        | "baseten"
        | "ollama"
        | "lm-studio"
        | "vllm" => DiscoveryAdapter::OpenAi,
        "anthropic" => DiscoveryAdapter::Anthropic,
        "google" => DiscoveryAdapter::Gemini,
        "cohere" => DiscoveryAdapter::Cohere,
        "fireworks" => DiscoveryAdapter::Fireworks,
        _ => return None,
    };
    let credential = if matches!(
        provider_type,
        "openrouter"
            | "alibaba-coding-plan-cn"
            | "alibaba-coding-plan"
            | "nvidia"
            | "poe"
            | "huggingface"
            | "modelscope"
            | "requesty"
            | "ollama"
            | "lm-studio"
            | "vllm"
    ) {
        CredentialPolicy::Optional
    } else {
        CredentialPolicy::Required
    };
    Some(DiscoverySpec {
        adapter,
        credential,
    })
}

fn discovery_spec_for_provider(
    provider: &ModelProviderConfig,
) -> Result<DiscoverySpec, String> {
    if let Some(spec) = reviewed_discovery_spec(&provider.provider) {
        return Ok(spec);
    }
    let adapter = if provider
        .protocols
        .contains_key(&ModelProtocol::OpenaiResponses)
        || provider
            .protocols
            .contains_key(&ModelProtocol::OpenaiCompletions)
    {
        DiscoveryAdapter::OpenAi
    } else if provider
        .protocols
        .contains_key(&ModelProtocol::AnthropicMessages)
    {
        DiscoveryAdapter::Anthropic
    } else if provider
        .protocols
        .contains_key(&ModelProtocol::GeminiGenerateContent)
    {
        DiscoveryAdapter::Gemini
    } else {
        return Err(
            "model_discovery_endpoint_invalid: Provider has no configured protocol endpoint"
                .into(),
        );
    };
    Ok(DiscoverySpec {
        adapter,
        credential: CredentialPolicy::Optional,
    })
}

fn discovery_url(provider: &ModelProviderConfig, adapter: DiscoveryAdapter) -> Result<Url, String> {
    match adapter {
        DiscoveryAdapter::OpenAi => [
            ModelProtocol::OpenaiResponses,
            ModelProtocol::OpenaiCompletions,
        ]
        .into_iter()
        .find_map(|protocol| {
            provider
                .protocols
                .get(&protocol)
                .map(|_| derived_protocol_url(provider, protocol, "/models"))
        })
        .unwrap_or_else(|| {
            Err("model_discovery_endpoint_invalid: Provider has no reviewed OpenAI endpoint".into())
        }),
        DiscoveryAdapter::Anthropic => {
            let mut url =
                derived_protocol_url(provider, ModelProtocol::AnthropicMessages, "/v1/models")?;
            url.query_pairs_mut().append_pair("limit", "1000");
            Ok(url)
        }
        DiscoveryAdapter::Gemini => {
            let mut url =
                derived_protocol_url(provider, ModelProtocol::GeminiGenerateContent, "/models")?;
            url.query_pairs_mut().append_pair("pageSize", "1000");
            Ok(url)
        }
        DiscoveryAdapter::Cohere => fixed_origin_url(provider, "/v1/models"),
        DiscoveryAdapter::Fireworks => {
            let mut url = fixed_origin_url(provider, "/v1/accounts/fireworks/models")?;
            url.query_pairs_mut()
                .append_pair("filter", "supports_serverless=true")
                .append_pair("pageSize", "200");
            Ok(url)
        }
    }
}

fn derived_protocol_url(
    provider: &ModelProviderConfig,
    protocol: ModelProtocol,
    replacement: &str,
) -> Result<Url, String> {
    let endpoint = provider.protocols.get(&protocol).ok_or_else(|| {
        "model_discovery_endpoint_invalid: Provider has no reviewed protocol endpoint".to_owned()
    })?;
    let request_url = full_request_url(&provider.base_url, &endpoint.endpoint_path)
        .map_err(|_| "model_discovery_endpoint_invalid: Provider endpoint is invalid".to_owned())?;
    let operation = protocol.default_endpoint_path();
    let prefix = request_url.strip_suffix(operation).ok_or_else(|| {
        "model_discovery_endpoint_invalid: Provider endpoint does not end with the reviewed protocol operation"
            .to_owned()
    })?;
    Url::parse(&format!("{prefix}{replacement}")).map_err(|_| {
        "model_discovery_endpoint_invalid: Provider catalog URL could not be derived".into()
    })
}

fn fixed_origin_url(provider: &ModelProviderConfig, path: &str) -> Result<Url, String> {
    let mut url = Url::parse(&provider.base_url)
        .map_err(|_| "model_discovery_endpoint_invalid: Provider Base URL is invalid".to_owned())?;
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn decode_page(adapter: DiscoveryAdapter, value: Value) -> Result<DecodedPage, String> {
    let next_token = match adapter {
        DiscoveryAdapter::Anthropic
            if value.get("has_more").and_then(Value::as_bool) == Some(true) =>
        {
            required_token(value.get("last_id"))?
        }
        DiscoveryAdapter::Gemini => optional_token(value.get("nextPageToken"))?,
        DiscoveryAdapter::Cohere => optional_token(
            value
                .get("next_page_token")
                .or_else(|| value.get("nextPageToken")),
        )?,
        DiscoveryAdapter::Fireworks => optional_token(value.get("nextPageToken"))?,
        _ => None,
    };
    let entries = match adapter {
        DiscoveryAdapter::OpenAi => {
            if let Some(entries) = value.as_array() {
                entries
            } else {
                value.get("data").and_then(Value::as_array).ok_or_else(|| {
                    "model_discovery_invalid_response: Provider returned an invalid model catalog"
                        .to_owned()
                })?
            }
        }
        DiscoveryAdapter::Anthropic => {
            value.get("data").and_then(Value::as_array).ok_or_else(|| {
                "model_discovery_invalid_response: Anthropic returned an invalid model catalog"
                    .to_owned()
            })?
        }
        DiscoveryAdapter::Gemini | DiscoveryAdapter::Cohere | DiscoveryAdapter::Fireworks => value
            .get("models")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                "model_discovery_invalid_response: Provider returned an invalid model catalog"
                    .to_owned()
            })?,
    };

    let models = entries
        .iter()
        .filter(|entry| {
            adapter != DiscoveryAdapter::Gemini
                || entry
                    .get("supportedGenerationMethods")
                    .and_then(Value::as_array)
                    .is_some_and(|methods| {
                        methods
                            .iter()
                            .any(|method| method.as_str() == Some("generateContent"))
                    })
        })
        .filter_map(|entry| model_summary(adapter, entry))
        .collect();
    Ok(DecodedPage { models, next_token })
}

fn model_summary(adapter: DiscoveryAdapter, value: &Value) -> Option<ProviderModelSummary> {
    let id = match adapter {
        DiscoveryAdapter::OpenAi | DiscoveryAdapter::Anthropic => string_field(value, &["id"]),
        DiscoveryAdapter::Gemini => string_field(value, &["baseModelId"]).or_else(|| {
            string_field(value, &["name"]).map(|name| {
                name.strip_prefix("models/")
                    .unwrap_or(name.as_str())
                    .to_owned()
            })
        }),
        DiscoveryAdapter::Cohere | DiscoveryAdapter::Fireworks => string_field(value, &["name"]),
    }?;
    let name = string_field(value, &["display_name", "displayName", "title", "name"]);
    let context_length = integer_field(
        value,
        &[
            "context_length",
            "context_window",
            "context_size",
            "max_context_length",
            "max_input_tokens",
            "inputTokenLimit",
            "contextLength",
        ],
    );
    Some(ProviderModelSummary {
        id,
        name,
        context_length,
    })
}

fn string_field(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?.as_str()?.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn integer_field(value: &Value, fields: &[&str]) -> Option<u64> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?;
        value
            .as_u64()
            .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
    })
}

fn optional_token(value: Option<&Value>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let token = value.as_str().ok_or_else(|| {
        "model_discovery_invalid_response: Provider returned an invalid continuation token"
            .to_owned()
    })?;
    let token = token.trim();
    if token.is_empty() {
        return Ok(None);
    }
    if token.len() > 2_048 || token.chars().any(char::is_control) {
        return Err(
            "model_discovery_invalid_response: Provider returned an invalid continuation token"
                .into(),
        );
    }
    Ok(Some(token.to_owned()))
}

fn required_token(value: Option<&Value>) -> Result<Option<String>, String> {
    optional_token(value)?.map(Some).ok_or_else(|| {
        "model_discovery_invalid_response: Provider omitted its continuation token".into()
    })
}

fn merge_models(
    target: &mut Vec<ProviderModelSummary>,
    incoming: Vec<ProviderModelSummary>,
) -> Result<(), String> {
    let mut seen = target
        .iter()
        .map(|model| model.id.clone())
        .collect::<BTreeSet<_>>();
    for mut model in incoming {
        model.id = model.id.trim().to_owned();
        if model.id.is_empty() || !seen.insert(model.id.clone()) {
            continue;
        }
        if target.len() == MAX_MODELS {
            return Err(format!(
                "model_discovery_too_many_models: Provider catalog exceeded the {MAX_MODELS}-model safety limit"
            ));
        }
        model.name = model
            .name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        target.push(model);
    }
    Ok(())
}

fn read_bounded(mut reader: impl Read, maximum: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let remaining = maximum.saturating_add(1).saturating_sub(total);
        let requested = buffer.len().min(remaining as usize);
        if requested == 0 {
            return Err(format!(
                "model_discovery_response_too_large: Provider response exceeded the {maximum}-byte safety limit"
            ));
        }
        let read = reader.read(&mut buffer[..requested]).map_err(|_| {
            "model_discovery_network: Provider response ended unexpectedly".to_owned()
        })?;
        if read == 0 {
            return Ok(bytes);
        }
        total = total.saturating_add(read as u64);
        if total > maximum {
            return Err(format!(
                "model_discovery_response_too_large: Provider response exceeded the {maximum}-byte safety limit"
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
}

fn discovery_agent() -> Result<ureq::Agent, String> {
    crate::network::build_ureq_agent(
        ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(15)))
            .user_agent("mux-provider-model-discovery"),
    )
}

fn fetch_page(
    agent: &ureq::Agent,
    url: &Url,
    adapter: DiscoveryAdapter,
    credential: Option<&str>,
) -> Result<DecodedPage, String> {
    let authorization = matches!(
        adapter,
        DiscoveryAdapter::OpenAi | DiscoveryAdapter::Cohere | DiscoveryAdapter::Fireworks
    )
    .then(|| credential.map(|credential| Zeroizing::new(format!("Bearer {credential}"))))
    .flatten();
    let mut request = agent.get(url.as_str()).header("Accept", "application/json");
    if let Some(authorization) = authorization.as_ref() {
        request = request.header("Authorization", authorization.as_str());
    }
    if let Some(credential) = credential {
        match adapter {
            DiscoveryAdapter::Anthropic => {
                request = request
                    .header("X-Api-Key", credential)
                    .header("anthropic-version", "2023-06-01");
            }
            DiscoveryAdapter::Gemini => {
                request = request.header("x-goog-api-key", credential);
            }
            _ => {}
        }
    }
    let mut response = request.call().map_err(|_| {
        "model_discovery_network: Unable to reach the Provider model catalog".to_owned()
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!(
            "model_discovery_http: Provider model catalog returned HTTP {status}"
        ));
    }
    let bytes = read_bounded(response.body_mut().as_reader(), MAX_RESPONSE_BYTES)?;
    let value = serde_json::from_slice(&bytes).map_err(|_| {
        "model_discovery_invalid_response: Provider returned invalid JSON".to_owned()
    })?;
    decode_page(adapter, value)
}

fn next_page_url(mut url: Url, adapter: DiscoveryAdapter, token: &str) -> Result<Url, String> {
    let key = match adapter {
        DiscoveryAdapter::Anthropic => "after_id",
        DiscoveryAdapter::Gemini | DiscoveryAdapter::Fireworks => "pageToken",
        DiscoveryAdapter::Cohere => "page_token",
        DiscoveryAdapter::OpenAi => {
            return Err(
                "model_discovery_invalid_response: OpenAI-compatible catalog returned an unsupported continuation token"
                    .into(),
            )
        }
    };
    let existing = url
        .query_pairs()
        .filter(|(name, _)| name != key)
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    {
        let mut query = url.query_pairs_mut();
        query.extend_pairs(existing);
        query.append_pair(key, token);
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{ModelProtocol, ModelProviderProtocolConfig};
    use crate::testenv::TestHome;
    use std::collections::BTreeMap;
    use std::io::{Cursor, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    const EXPECTED_OPENAI_COMPATIBLE: &[&str] = &[
        "openrouter",
        "openai",
        "xai",
        "mistral",
        "deepseek",
        "groq",
        "alibaba",
        "alibaba-coding-plan-cn",
        "alibaba-coding-plan",
        "alibaba-token-plan-cn",
        "alibaba-token-plan",
        "xiaomi",
        "xiaomi-token-plan-cn",
        "xiaomi-token-plan-sgp",
        "xiaomi-token-plan-ams",
        "moonshotai",
        "kimi-for-coding",
        "zai",
        "zai-coding-plan",
        "zhipuai-coding-plan",
        "minimax-coding-plan",
        "minimax-cn-coding-plan",
        "stepfun-step-plan",
        "stepfun-ai-step-plan",
        "tencent-coding-plan",
        "tencent-token-plan",
        "tencent-token-plan-global",
        "nvidia",
        "cerebras",
        "siliconflow",
        "together",
        "poe",
        "huggingface",
        "novita-ai",
        "qiniu-ai",
        "digitalocean",
        "modelscope",
        "scaleway",
        "nebius",
        "requesty",
        "baseten",
        "ollama",
        "lm-studio",
        "vllm",
    ];

    fn provider(
        provider_type: &str,
        base_url: &str,
        protocol: ModelProtocol,
        endpoint_path: &str,
    ) -> ModelProviderConfig {
        ModelProviderConfig {
            id: format!("{provider_type}-instance"),
            name: format!("{provider_type} instance"),
            provider: provider_type.into(),
            base_url: base_url.into(),
            protocols: BTreeMap::from([(
                protocol,
                ModelProviderProtocolConfig {
                    endpoint_path: endpoint_path.into(),
                },
            )]),
            env_key: None,
        }
    }

    fn serve_once(
        status: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> (Url, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        let status = status.to_owned();
        let headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>();
        let body = body.to_owned();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            sender
                .send(String::from_utf8_lossy(&request).into_owned())
                .unwrap();
            let mut response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
                body.len(),
            );
            for (name, value) in headers {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str("\r\n");
            response.push_str(&body);
            stream.write_all(response.as_bytes()).unwrap();
        });
        (
            Url::parse(&format!("http://{address}/models")).unwrap(),
            receiver,
            handle,
        )
    }

    #[test]
    fn allows_every_builtin_and_custom_provider_to_attempt_model_discovery() {
        assert!(super::super::MODEL_PROVIDERS
            .iter()
            .all(|provider| model_discovery_supported(provider.id)));
        for provider_type in ["github-models", "wandb", "custom", "private-gateway"] {
            assert!(model_discovery_supported(provider_type), "{provider_type}");
        }
    }

    #[test]
    fn assigns_the_expected_adapter_and_credential_policy() {
        for provider_type in EXPECTED_OPENAI_COMPATIBLE {
            assert_eq!(
                reviewed_discovery_spec(provider_type).map(|spec| spec.adapter),
                Some(DiscoveryAdapter::OpenAi),
                "{provider_type}",
            );
        }
        assert_eq!(
            reviewed_discovery_spec("anthropic").map(|spec| spec.adapter),
            Some(DiscoveryAdapter::Anthropic),
        );
        assert_eq!(
            reviewed_discovery_spec("google").map(|spec| spec.adapter),
            Some(DiscoveryAdapter::Gemini),
        );
        assert_eq!(
            reviewed_discovery_spec("cohere").map(|spec| spec.adapter),
            Some(DiscoveryAdapter::Cohere),
        );
        assert_eq!(
            reviewed_discovery_spec("fireworks").map(|spec| spec.adapter),
            Some(DiscoveryAdapter::Fireworks),
        );

        for provider_type in [
            "openrouter",
            "alibaba-coding-plan-cn",
            "alibaba-coding-plan",
            "nvidia",
            "poe",
            "huggingface",
            "modelscope",
            "requesty",
            "ollama",
            "lm-studio",
            "vllm",
        ] {
            assert_eq!(
                reviewed_discovery_spec(provider_type).map(|spec| spec.credential),
                Some(CredentialPolicy::Optional),
                "{provider_type}",
            );
        }
        assert_eq!(
            reviewed_discovery_spec("openai").map(|spec| spec.credential),
            Some(CredentialPolicy::Required),
        );
    }

    #[test]
    fn derives_optional_generic_discovery_from_custom_provider_protocols() {
        let openai = provider(
            "custom",
            "https://gateway.example.test",
            ModelProtocol::OpenaiResponses,
            "/tenant/v1/responses",
        );
        let openai_spec = discovery_spec_for_provider(&openai).unwrap();
        assert_eq!(openai_spec.adapter, DiscoveryAdapter::OpenAi);
        assert_eq!(openai_spec.credential, CredentialPolicy::Optional);
        assert_eq!(
            discovery_url(&openai, openai_spec.adapter).unwrap().as_str(),
            "https://gateway.example.test/tenant/v1/models",
        );

        let anthropic = provider(
            "private-anthropic",
            "https://gateway.example.test",
            ModelProtocol::AnthropicMessages,
            "/tenant/v1/messages",
        );
        let anthropic_spec = discovery_spec_for_provider(&anthropic).unwrap();
        assert_eq!(anthropic_spec.adapter, DiscoveryAdapter::Anthropic);
        assert_eq!(anthropic_spec.credential, CredentialPolicy::Optional);
        assert_eq!(
            discovery_url(&anthropic, anthropic_spec.adapter)
                .unwrap()
                .as_str(),
            "https://gateway.example.test/tenant/v1/models?limit=1000",
        );

        let gemini = provider(
            "private-gemini",
            "https://gateway.example.test/v1beta",
            ModelProtocol::GeminiGenerateContent,
            "/models/{model}:generateContent",
        );
        let gemini_spec = discovery_spec_for_provider(&gemini).unwrap();
        assert_eq!(gemini_spec.adapter, DiscoveryAdapter::Gemini);
        assert_eq!(gemini_spec.credential, CredentialPolicy::Optional);
        assert_eq!(
            discovery_url(&gemini, gemini_spec.adapter).unwrap().as_str(),
            "https://gateway.example.test/v1beta/models?pageSize=1000",
        );
    }

    #[test]
    fn derives_only_reviewed_same_origin_catalog_urls() {
        let openrouter = provider(
            "openrouter",
            "https://openrouter.ai",
            ModelProtocol::OpenaiResponses,
            "/api/v1/responses",
        );
        assert_eq!(
            discovery_url(&openrouter, DiscoveryAdapter::OpenAi)
                .unwrap()
                .as_str(),
            "https://openrouter.ai/api/v1/models",
        );

        let anthropic = provider(
            "anthropic",
            "https://api.anthropic.com",
            ModelProtocol::AnthropicMessages,
            "/v1/messages",
        );
        assert_eq!(
            discovery_url(&anthropic, DiscoveryAdapter::Anthropic)
                .unwrap()
                .as_str(),
            "https://api.anthropic.com/v1/models?limit=1000",
        );

        let gemini = provider(
            "google",
            "https://generativelanguage.googleapis.com/v1beta",
            ModelProtocol::GeminiGenerateContent,
            "/models/{model}:generateContent",
        );
        assert_eq!(
            discovery_url(&gemini, DiscoveryAdapter::Gemini)
                .unwrap()
                .as_str(),
            "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000",
        );

        let cohere = provider(
            "cohere",
            "https://api.cohere.ai/compatibility/v1",
            ModelProtocol::OpenaiCompletions,
            "/chat/completions",
        );
        assert_eq!(
            discovery_url(&cohere, DiscoveryAdapter::Cohere)
                .unwrap()
                .as_str(),
            "https://api.cohere.ai/v1/models",
        );

        let fireworks = provider(
            "fireworks",
            "https://api.fireworks.ai/inference/v1",
            ModelProtocol::OpenaiCompletions,
            "/chat/completions",
        );
        assert_eq!(
            discovery_url(&fireworks, DiscoveryAdapter::Fireworks)
                .unwrap()
                .as_str(),
            "https://api.fireworks.ai/v1/accounts/fireworks/models?filter=supports_serverless%3Dtrue&pageSize=200",
        );
    }

    #[test]
    fn normalizes_all_response_shapes_without_overwriting_first_seen_models() {
        let mut models = decode_page(
            DiscoveryAdapter::OpenAi,
            serde_json::json!({
                "data": [
                    {"id": "alpha", "name": "Alpha", "context_length": 128000},
                    {"id": "", "name": "Ignored"}
                ]
            }),
        )
        .unwrap()
        .models;
        merge_models(
            &mut models,
            decode_page(
                DiscoveryAdapter::OpenAi,
                serde_json::json!([
                    {"id": "alpha", "name": "Replacement"},
                    {"id": "beta", "display_name": "Beta", "context_window": 64000}
                ]),
            )
            .unwrap()
            .models,
        )
        .unwrap();

        assert_eq!(
            models,
            vec![
                ProviderModelSummary {
                    id: "alpha".into(),
                    name: Some("Alpha".into()),
                    context_length: Some(128000),
                },
                ProviderModelSummary {
                    id: "beta".into(),
                    name: Some("Beta".into()),
                    context_length: Some(64000),
                },
            ],
        );
    }

    #[test]
    fn native_adapters_extract_ids_capabilities_and_continuation_tokens() {
        let anthropic = decode_page(
            DiscoveryAdapter::Anthropic,
            serde_json::json!({
                "data": [{"id": "claude-sonnet", "display_name": "Claude Sonnet", "max_input_tokens": 200000}],
                "has_more": true,
                "last_id": "claude-sonnet"
            }),
        )
        .unwrap();
        assert_eq!(anthropic.next_token.as_deref(), Some("claude-sonnet"));
        assert_eq!(anthropic.models[0].id, "claude-sonnet");

        let gemini = decode_page(
            DiscoveryAdapter::Gemini,
            serde_json::json!({
                "models": [
                    {"name": "models/gemini-2.5-pro", "baseModelId": "gemini-2.5-pro", "displayName": "Gemini 2.5 Pro", "inputTokenLimit": 1048576, "supportedGenerationMethods": ["generateContent"]},
                    {"name": "models/embedding-001", "supportedGenerationMethods": ["embedContent"]}
                ],
                "nextPageToken": "next-gemini"
            }),
        )
        .unwrap();
        assert_eq!(gemini.next_token.as_deref(), Some("next-gemini"));
        assert_eq!(gemini.models.len(), 1);
        assert_eq!(gemini.models[0].id, "gemini-2.5-pro");

        let cohere = decode_page(
            DiscoveryAdapter::Cohere,
            serde_json::json!({"models": [{"name": "command-r-plus", "context_length": 128000}], "next_page_token": "next-cohere"}),
        )
        .unwrap();
        assert_eq!(cohere.next_token.as_deref(), Some("next-cohere"));
        assert_eq!(cohere.models[0].id, "command-r-plus");

        let fireworks = decode_page(
            DiscoveryAdapter::Fireworks,
            serde_json::json!({"models": [{"name": "accounts/fireworks/models/llama-v3", "displayName": "Llama 3", "contextLength": 8192}], "nextPageToken": "next-fireworks"}),
        )
        .unwrap();
        assert_eq!(fireworks.next_token.as_deref(), Some("next-fireworks"));
        assert_eq!(fireworks.models[0].id, "accounts/fireworks/models/llama-v3",);
    }

    #[test]
    fn enforces_response_page_and_model_limits() {
        assert_eq!(
            read_bounded(Cursor::new(vec![b'x'; 16]), 16).unwrap().len(),
            16
        );
        assert!(read_bounded(Cursor::new(vec![b'x'; 17]), 16)
            .unwrap_err()
            .starts_with("model_discovery_response_too_large:"));
        assert_eq!(MAX_RESPONSE_BYTES, 4 * 1024 * 1024);
        assert_eq!(MAX_MODELS, 2_000);
        assert_eq!(MAX_PAGES, 10);

        let mut models = (0..MAX_MODELS)
            .map(|index| ProviderModelSummary {
                id: format!("model-{index}"),
                name: None,
                context_length: None,
            })
            .collect::<Vec<_>>();
        let error = merge_models(
            &mut models,
            vec![ProviderModelSummary {
                id: "one-too-many".into(),
                name: None,
                context_length: None,
            }],
        )
        .unwrap_err();
        assert!(
            error.starts_with("model_discovery_too_many_models:"),
            "{error}"
        );
    }

    #[test]
    fn sends_only_the_adapter_specific_authentication_headers() {
        let _home = TestHome::new("model-discovery-auth-headers");
        let agent = discovery_agent().unwrap();

        let (url, request, handle) = serve_once("200 OK", &[], r#"{"data": []}"#);
        fetch_page(
            &agent,
            &url,
            DiscoveryAdapter::OpenAi,
            Some("openai-secret"),
        )
        .unwrap();
        let request = request.recv().unwrap().to_ascii_lowercase();
        handle.join().unwrap();
        assert!(request.contains("authorization: bearer openai-secret\r\n"));

        let (url, request, handle) = serve_once("200 OK", &[], r#"{"data": []}"#);
        fetch_page(
            &agent,
            &url,
            DiscoveryAdapter::Anthropic,
            Some("anthropic-secret"),
        )
        .unwrap();
        let request = request.recv().unwrap().to_ascii_lowercase();
        handle.join().unwrap();
        assert!(request.contains("x-api-key: anthropic-secret\r\n"));
        assert!(request.contains("anthropic-version: 2023-06-01\r\n"));
        assert!(!request.contains("authorization:"));

        let (url, request, handle) = serve_once("200 OK", &[], r#"{"models": []}"#);
        fetch_page(
            &agent,
            &url,
            DiscoveryAdapter::Gemini,
            Some("gemini-secret"),
        )
        .unwrap();
        let request = request.recv().unwrap().to_ascii_lowercase();
        handle.join().unwrap();
        assert!(request.contains("x-goog-api-key: gemini-secret\r\n"));
        assert!(!request.contains("authorization:"));
    }

    #[test]
    fn refuses_redirects_and_never_echoes_provider_bodies_or_credentials() {
        let _home = TestHome::new("model-discovery-safe-errors");
        let agent = discovery_agent().unwrap();
        let (url, _request, handle) = serve_once(
            "302 Found",
            &[("Location", "http://127.0.0.1:9/credential-sink")],
            "redirected openai-secret",
        );
        let redirect = fetch_page(
            &agent,
            &url,
            DiscoveryAdapter::OpenAi,
            Some("openai-secret"),
        )
        .unwrap_err();
        handle.join().unwrap();
        assert!(redirect.starts_with("model_discovery_http:"), "{redirect}");
        assert!(redirect.contains("302"), "{redirect}");
        assert!(!redirect.contains("credential-sink"), "{redirect}");
        assert!(!redirect.contains("openai-secret"), "{redirect}");

        let (url, _request, handle) = serve_once(
            "401 Unauthorized",
            &[],
            "provider echoed openai-secret in its body",
        );
        let unauthorized = fetch_page(
            &agent,
            &url,
            DiscoveryAdapter::OpenAi,
            Some("openai-secret"),
        )
        .unwrap_err();
        handle.join().unwrap();
        assert!(
            unauthorized.starts_with("model_discovery_http:"),
            "{unauthorized}"
        );
        assert!(unauthorized.contains("401"), "{unauthorized}");
        assert!(!unauthorized.contains("provider echoed"), "{unauthorized}");
        assert!(!unauthorized.contains("openai-secret"), "{unauthorized}");
    }
}
