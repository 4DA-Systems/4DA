// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Model evaluation and synthesis capability checking.

use crate::error::Result;

/// Implementation for check_synthesis_capability command.
pub(super) async fn check_synthesis_capability_impl() -> Result<serde_json::Value> {
    let llm_settings = {
        let settings = crate::get_settings_manager().lock();
        settings.get().llm.clone()
    };

    let capable = crate::ollama::can_synthesize(&llm_settings).await;
    let can_explain = crate::ollama::can_explain(&llm_settings).await;

    let (model_name, model_params, provider) = if llm_settings.provider == "ollama" {
        let base_url = llm_settings
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let model = llm_settings.model.clone();
        let params = crate::ollama::get_model_params_billions(&model, base_url).await;
        (model, params, "ollama".to_string())
    } else if llm_settings.provider == "builtin" {
        (llm_settings.model.clone(), None, "builtin".to_string())
    } else {
        (
            llm_settings.model.clone(),
            None,
            llm_settings.provider.clone(),
        )
    };

    let hw = crate::hardware_detect::detect_hardware();
    let tier = crate::hardware_detect::ram_tier(&hw);

    let model_tier = if provider == "ollama" && !model_name.is_empty() {
        Some(crate::model_allowlist::classify_model(&model_name))
    } else {
        None
    };

    let recommended = crate::model_allowlist::recommend_models(hw.ram_total_gb);
    let top_recommendation = recommended.first().map(|e| e.family);

    let guidance = if provider == "builtin" {
        if crate::llm_engine::sidecar_status() == crate::llm_engine::SidecarStatus::Ready {
            "Built-in LLM is running and ready for synthesis.".to_string()
        } else {
            "Built-in LLM sidecar is not running — start it from settings.".to_string()
        }
    } else if capable {
        "Your model supports AI-powered briefing synthesis.".to_string()
    } else if model_name.is_empty() {
        match top_recommendation {
            Some(rec) => format!(
                "No model selected. Based on your hardware ({:.0} GB RAM), we recommend: ollama pull {rec}",
                hw.ram_total_gb
            ),
            None => "No model selected. Configure a cloud API key (Anthropic/OpenAI) — your system RAM is too low for local models.".to_string(),
        }
    } else {
        match top_recommendation {
            Some(rec) => format!(
                "Your model is below the synthesis threshold. Based on your hardware ({:.0} GB RAM), try: ollama pull {rec}",
                hw.ram_total_gb
            ),
            None => "Briefing synthesis requires a 7B+ parameter model or a cloud API key (Anthropic/OpenAI).".to_string(),
        }
    };

    Ok(serde_json::json!({
        "can_synthesize": capable,
        "can_explain": can_explain,
        "provider": provider,
        "model": model_name,
        "params_billions": model_params,
        "min_params_billions": 7.0,
        "unverified": provider == "openai-compatible",
        "model_tier": model_tier,
        "guidance": guidance,
        "hardware": {
            "ram_total_gb": hw.ram_total_gb,
            "ram_available_gb": hw.ram_available_gb,
            "ram_tier": tier,
            "gpu": hw.gpu,
        },
        "recommended_model": top_recommendation,
    }))
}

/// Implementation for run_model_eval command.
pub(super) async fn run_model_eval_impl() -> Result<serde_json::Value> {
    let llm_settings = {
        let mut guard = crate::get_settings_manager().lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };

    let summary = crate::model_eval::run_eval(&llm_settings.model, &llm_settings.provider)
        .await
        .map_err(|e| e.to_string())?;

    serde_json::to_value(&summary).map_err(|e| e.to_string().into())
}
