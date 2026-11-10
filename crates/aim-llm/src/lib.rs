//! Language layer: an LLM only rewrites numbers the deterministic engine already produced.
//! Every answer is checked; any failure falls back to the fixed templates, so the app works with no LLM.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use thiserror::Error;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_OUTPUT_TOKENS: u32 = 2000;
const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2026-10-21";

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("could not reach the model: {0}")]
    Http(#[from] reqwest::Error),
    #[error("the model service returned {0}: {1}")]
    Status(u16, String),
    #[error("the model did not answer: {0}")]
    NoAnswer(String),
}

pub trait LlmProvider: Send + Sync {
    fn complete(&self, system: &str, user: &str) -> Result<String, LlmError>;
}

fn client() -> Result<reqwest::blocking::Client, LlmError> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?)
}

fn read_json(response: reqwest::blocking::Response) -> Result<Value, LlmError> {
    let status = response.status();
    let body = response.text()?;
    if !status.is_success() {
        return Err(LlmError::Status(status.as_u16(), body));
    }
    serde_json::from_str(&body).map_err(|e| LlmError::NoAnswer(e.to_string()))
}

/// Any OpenAI-compatible server: Ollama, LM Studio, llama.cpp, or a hosted API.
pub struct OpenAiCompatProvider {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl LlmProvider for OpenAiCompatProvider {
    fn complete(&self, system: &str, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut request = client()?.post(url).json(&json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        }));
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        read_json(request.send()?)?["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| LlmError::NoAnswer("empty response".to_string()))
    }
}

/// Claude Messages API. The key comes from the environment and is never stored by the app.
pub struct AnthropicProvider {
    pub api_key: String,
    pub model: String,
}

impl LlmProvider for AnthropicProvider {
    fn complete(&self, system: &str, user: &str) -> Result<String, LlmError> {
        let response = client()?
            .post(ANTHROPIC_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&json!({
                "model": self.model,
                "max_tokens": MAX_OUTPUT_TOKENS,
                "system": system,
                "messages": [{"role": "user", "content": user}],
            }))
            .send()?;
        let value = read_json(response)?;
        if value["stop_reason"] == "refusal" {
            return Err(LlmError::NoAnswer("the model declined".to_string()));
        }
        let text: String = value["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        if text.is_empty() {
            return Err(LlmError::NoAnswer("empty response".to_string()));
        }
        Ok(text)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weakness {
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub fix: String,
}

/// A coach report: the coach reads the player's data and writes this, the player does not chat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachReply {
    pub summary: String,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub weaknesses: Vec<Weakness>,
    #[serde(default)]
    pub next_steps: Vec<String>,
}

const EASTERN_DIGITS: [char; 10] = ['٠', '١', '٢', '٣', '٤', '٥', '٦', '٧', '٨', '٩'];

fn normalize_digits(text: &str) -> String {
    text.chars()
        .map(|c| match EASTERN_DIGITS.iter().position(|d| *d == c) {
            Some(i) => char::from_digit(i as u32, 10).unwrap_or(c),
            None if c == '٫' => '.',
            None => c,
        })
        .collect()
}

fn numbers_in(text: &str) -> Vec<String> {
    let normalized = normalize_digits(text);
    let mut out = Vec::new();
    let mut current = String::new();
    for c in normalized.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || (c == '.' && !current.is_empty()) {
            current.push(c);
        } else if !current.is_empty() {
            out.push(current.trim_end_matches('.').to_string());
            current.clear();
        }
    }
    out
}

fn collect_numbers(value: &Value, out: &mut Vec<f64>) {
    match value {
        Value::Number(n) => out.extend(n.as_f64()),
        Value::Array(items) => items.iter().for_each(|v| collect_numbers(v, out)),
        Value::Object(map) => map.values().for_each(|v| collect_numbers(v, out)),
        Value::String(s) => out.extend(numbers_in(s).iter().filter_map(|n| n.parse::<f64>().ok())),
        _ => {}
    }
}

/// True when every number in `text` exists in `input`, at the precision the text used.
/// Negatives and percentages are compared by magnitude, so "13%" also matches an input of 0.13.
pub fn numbers_are_grounded(text: &str, input: &Value) -> bool {
    let mut known = Vec::new();
    collect_numbers(input, &mut known);
    numbers_in(text).iter().all(|written| {
        let Ok(value) = written.parse::<f64>() else {
            return false;
        };
        let decimals = written.split('.').nth(1).map_or(0, str::len) as i32;
        let scale = 10_f64.powi(decimals);
        known.iter().any(|k| {
            let k = k.abs();
            (k * scale).round() == (value * scale).round()
                || (k * 100.0 * scale).round() == (value * scale).round()
        })
    })
}

fn extract_json(text: &str) -> Option<&str> {
    let (start, end) = (text.find('{')?, text.rfind('}')?);
    (start < end).then(|| &text[start..=end])
}

const SYSTEM_PROMPT: &str = "You are the coach inside AimForge, an aim trainer for VALORANT players. \
You receive measurements that are already final. Study them and write a report: what the player does well, \
what is holding them back and why, and exactly what to practise next, connected to real rounds (peeks, \
holds, retakes). Write in the language named by \"locale\". Use only numbers that appear in the data, and \
never invent a score or a problem. If the data is too thin to judge something, say so. Reply with one JSON \
object and nothing else, with the keys \"summary\" (string), \"strengths\" (array of strings), \
\"weaknesses\" (array of objects with \"title\", \"detail\", \"fix\"), \"next_steps\" (array of strings).";

/// Asks the model for a report and validates it. `Err` means the caller should show the built-in report.
pub fn write_report(
    provider: &dyn LlmProvider,
    input: &Value,
    extra_instructions: &str,
) -> Result<CoachReply, LlmError> {
    let system = if extra_instructions.trim().is_empty() {
        SYSTEM_PROMPT.to_string()
    } else {
        format!("{SYSTEM_PROMPT}\n\n{extra_instructions}")
    };
    let text = provider.complete(&system, &format!("Data:\n{input}"))?;
    let json_text =
        extract_json(&text).ok_or_else(|| LlmError::NoAnswer("no JSON in reply".to_string()))?;
    let reply: CoachReply = serde_json::from_str(json_text)
        .map_err(|e| LlmError::NoAnswer(format!("wrong reply shape: {e}")))?;
    let mut all_text = format!(
        "{} {} {}",
        reply.summary,
        reply.strengths.join(" "),
        reply.next_steps.join(" ")
    );
    for w in &reply.weaknesses {
        all_text.push_str(&format!(" {} {} {}", w.title, w.detail, w.fix));
    }
    if !numbers_are_grounded(&all_text, input) {
        return Err(LlmError::NoAnswer(
            "the reply used a number that is not in your data".to_string(),
        ));
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(&'static str);

    impl LlmProvider for Fixed {
        fn complete(&self, _: &str, _: &str) -> Result<String, LlmError> {
            Ok(self.0.to_string())
        }
    }

    fn input() -> Value {
        json!({"gain": 1.13, "delta": 14, "premature": 0.25, "days": 6})
    }

    #[test]
    fn grounded_numbers_pass() {
        assert!(numbers_are_grounded(
            "gain 1.13, up 14 points in 6 days",
            &input()
        ));
        assert!(numbers_are_grounded("25% early clicks", &input()));
        assert!(numbers_are_grounded("تحسنت ١٤ نقطة", &input()));
    }

    #[test]
    fn invented_numbers_fail() {
        assert!(!numbers_are_grounded("you improved 37 points", &input()));
    }

    #[test]
    fn a_valid_reply_inside_extra_text_is_accepted() {
        let p =
            Fixed(r#"Sure: {"summary":"up 14","strengths":[],"weaknesses":[],"next_steps":[]}"#);
        assert_eq!(write_report(&p, &input(), "").unwrap().summary, "up 14");
    }

    #[test]
    fn a_reply_with_an_invented_number_is_rejected() {
        let p = Fixed(r#"{"summary":"up 99","weaknesses":[]}"#);
        assert!(write_report(&p, &input(), "").is_err());
    }

    #[test]
    fn plain_text_is_rejected() {
        assert!(write_report(&Fixed("hello"), &input(), "").is_err());
    }
}
