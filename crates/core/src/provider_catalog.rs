use crate::provider_config::ProviderProtocol;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub protocol: ProviderProtocol,
    pub default_base_url: &'static str,
    pub default_api_key_env: Option<&'static str>,
}

macro_rules! openai_provider {
    ($id:literal, $name:literal, $url:literal, $key:expr) => {
        ProviderSpec {
            id: $id,
            name: $name,
            protocol: ProviderProtocol::OpenAiChat,
            default_base_url: $url,
            default_api_key_env: $key,
        }
    };
}

pub const PROVIDERS: &[ProviderSpec] = &[
    openai_provider!(
        "openai-compatible",
        "OpenAI-compatible",
        "http://127.0.0.1:8080/v1",
        None
    ),
    openai_provider!(
        "openrouter",
        "OpenRouter",
        "https://openrouter.ai/api/v1",
        Some("OPENROUTER_API_KEY")
    ),
    openai_provider!(
        "novita",
        "NovitaAI",
        "https://api.novita.ai/v3/openai",
        Some("NOVITA_API_KEY")
    ),
    openai_provider!("lm-studio", "LM Studio", "http://127.0.0.1:1234/v1", None),
    ProviderSpec {
        id: "anthropic",
        name: "Anthropic",
        protocol: ProviderProtocol::AnthropicMessages,
        default_base_url: "https://api.anthropic.com",
        default_api_key_env: Some("ANTHROPIC_API_KEY"),
    },
    openai_provider!(
        "openai",
        "OpenAI",
        "https://api.openai.com/v1",
        Some("OPENAI_API_KEY")
    ),
    openai_provider!(
        "qwen-cloud",
        "Qwen Cloud / DashScope",
        "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        Some("DASHSCOPE_API_KEY")
    ),
    openai_provider!(
        "xai",
        "xAI Grok",
        "https://api.x.ai/v1",
        Some("XAI_API_KEY")
    ),
    openai_provider!(
        "xiaomi-mimo",
        "Xiaomi MiMo",
        "https://api.xiaomimimo.com/v1",
        Some("MIMO_API_KEY")
    ),
    openai_provider!(
        "tencent-tokenhub",
        "Tencent TokenHub",
        "https://api.tokenhub.tencentmaas.com/v1",
        Some("TENCENT_TOKENHUB_API_KEY")
    ),
    openai_provider!(
        "nvidia-nim",
        "NVIDIA NIM",
        "https://integrate.api.nvidia.com/v1",
        Some("NVIDIA_API_KEY")
    ),
    openai_provider!(
        "github-models",
        "GitHub Models",
        "https://models.inference.ai.azure.com",
        Some("GITHUB_TOKEN")
    ),
    openai_provider!(
        "hugging-face",
        "Hugging Face Inference Providers",
        "https://router.huggingface.co/v1",
        Some("HF_TOKEN")
    ),
    ProviderSpec {
        id: "google-gemini",
        name: "Google Gemini",
        protocol: ProviderProtocol::GeminiGenerateContent,
        default_base_url: "https://generativelanguage.googleapis.com",
        default_api_key_env: Some("GEMINI_API_KEY"),
    },
    openai_provider!(
        "deepseek",
        "DeepSeek",
        "https://api.deepseek.com/v1",
        Some("DEEPSEEK_API_KEY")
    ),
    openai_provider!(
        "zai",
        "Z.AI / GLM",
        "https://api.z.ai/api/paas/v4",
        Some("ZAI_API_KEY")
    ),
    openai_provider!(
        "kimi",
        "Kimi / Moonshot",
        "https://api.moonshot.ai/v1",
        Some("MOONSHOT_API_KEY")
    ),
    openai_provider!(
        "stepfun",
        "StepFun Step Plan",
        "https://api.stepfun.com/v1",
        Some("STEPFUN_API_KEY")
    ),
    openai_provider!(
        "minimax",
        "MiniMax",
        "https://api.minimax.io/v1",
        Some("MINIMAX_API_KEY")
    ),
    openai_provider!(
        "ollama-cloud",
        "Ollama Cloud",
        "https://ollama.com/v1",
        Some("OLLAMA_API_KEY")
    ),
    openai_provider!(
        "arcee",
        "Arcee AI",
        "https://api.arcee.ai/v1",
        Some("ARCEE_API_KEY")
    ),
    openai_provider!(
        "gmi-cloud",
        "GMI Cloud",
        "https://api.gmi-serving.com/v1",
        Some("GMI_API_KEY")
    ),
    openai_provider!(
        "kilo-code",
        "Kilo Code",
        "https://api.kilo.ai/v1",
        Some("KILO_API_KEY")
    ),
    openai_provider!(
        "opencode",
        "OpenCode",
        "https://opencode.ai/zen/v1",
        Some("OPENCODE_API_KEY")
    ),
    openai_provider!(
        "aws-bedrock",
        "AWS Bedrock OpenAI endpoint",
        "http://127.0.0.1:8000/v1",
        Some("AWS_BEARER_TOKEN_BEDROCK")
    ),
    openai_provider!(
        "azure-foundry",
        "Azure Foundry",
        "https://example.services.ai.azure.com/models",
        Some("AZURE_AI_API_KEY")
    ),
    openai_provider!(
        "qwen-oauth",
        "Qwen OAuth bridge",
        "http://127.0.0.1:8080/v1",
        None
    ),
    openai_provider!(
        "alibaba-coding-plan",
        "Alibaba Cloud Coding Plan",
        "https://coding.dashscope.aliyuncs.com/v1",
        Some("DASHSCOPE_API_KEY")
    ),
    openai_provider!(
        "custom",
        "Custom direct API",
        "http://127.0.0.1:8080/v1",
        None
    ),
    openai_provider!(
        "custom-endpoint",
        "Custom endpoint",
        "http://127.0.0.1:8080/v1",
        None
    ),
];

pub fn find(provider_id: &str) -> Option<&'static ProviderSpec> {
    PROVIDERS.iter().find(|provider| provider.id == provider_id)
}

#[cfg(test)]
mod tests {
    use super::{find, PROVIDERS};

    #[test]
    fn catalog_ids_are_unique_and_expected_providers_exist() {
        let mut ids = PROVIDERS
            .iter()
            .map(|provider| provider.id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), PROVIDERS.len());
        assert_eq!(
            find("openrouter").map(|provider| provider.name),
            Some("OpenRouter")
        );
        assert_eq!(
            find("google-gemini").map(|provider| provider.name),
            Some("Google Gemini")
        );
    }
}
