use super::ModelProviderConfig;
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use url::Url;

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
    false
}

pub fn discover_provider_models(
    _provider_id: &str,
) -> Result<Vec<ProviderModelSummary>, String> {
    Err("model_discovery_not_implemented".into())
}

fn discovery_spec(_provider_type: &str) -> Option<DiscoverySpec> {
    None
}

fn discovery_url(
    _provider: &ModelProviderConfig,
    _adapter: DiscoveryAdapter,
) -> Result<Url, String> {
    Err("model_discovery_not_implemented".into())
}

fn decode_page(_adapter: DiscoveryAdapter, _value: Value) -> Result<DecodedPage, String> {
    Err("model_discovery_not_implemented".into())
}

fn merge_models(
    _target: &mut Vec<ProviderModelSummary>,
    _incoming: Vec<ProviderModelSummary>,
) -> Result<(), String> {
    Err("model_discovery_not_implemented".into())
}

fn read_bounded(_reader: impl Read, _maximum: u64) -> Result<Vec<u8>, String> {
    Err("model_discovery_not_implemented".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{ModelProtocol, ModelProviderProtocolConfig};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Cursor;

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

    #[test]
    fn supports_exactly_the_researched_48_builtin_provider_types() {
        let expected = EXPECTED_OPENAI_COMPATIBLE
            .iter()
            .copied()
            .chain(["anthropic", "google", "cohere", "fireworks"])
            .collect::<BTreeSet<_>>();
        let actual = super::super::MODEL_PROVIDERS
            .iter()
            .filter(|provider| model_discovery_supported(provider.id))
            .map(|provider| provider.id)
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 48);
        for unsupported in ["github-models", "wandb", "custom"] {
            assert!(!model_discovery_supported(unsupported));
        }
    }

    #[test]
    fn assigns_the_expected_adapter_and_credential_policy() {
        for provider_type in EXPECTED_OPENAI_COMPATIBLE {
            assert_eq!(
                discovery_spec(provider_type).map(|spec| spec.adapter),
                Some(DiscoveryAdapter::OpenAi),
                "{provider_type}",
            );
        }
        assert_eq!(
            discovery_spec("anthropic").map(|spec| spec.adapter),
            Some(DiscoveryAdapter::Anthropic),
        );
        assert_eq!(
            discovery_spec("google").map(|spec| spec.adapter),
            Some(DiscoveryAdapter::Gemini),
        );
        assert_eq!(
            discovery_spec("cohere").map(|spec| spec.adapter),
            Some(DiscoveryAdapter::Cohere),
        );
        assert_eq!(
            discovery_spec("fireworks").map(|spec| spec.adapter),
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
                discovery_spec(provider_type).map(|spec| spec.credential),
                Some(CredentialPolicy::Optional),
                "{provider_type}",
            );
        }
        assert_eq!(
            discovery_spec("openai").map(|spec| spec.credential),
            Some(CredentialPolicy::Required),
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
        assert_eq!(
            fireworks.models[0].id,
            "accounts/fireworks/models/llama-v3",
        );
    }

    #[test]
    fn enforces_response_page_and_model_limits() {
        assert_eq!(read_bounded(Cursor::new(vec![b'x'; 16]), 16).unwrap().len(), 16);
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
        assert!(error.starts_with("model_discovery_too_many_models:"), "{error}");
    }
}
