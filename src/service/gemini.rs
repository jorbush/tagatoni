use crate::model::recipe::Recipe;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

pub struct GeminiService {
    client: reqwest::Client,
    api_key: String,
}

#[derive(Serialize)]
struct GeminiRequest {
    model: String,
    input: String,
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f64,
    max_output_tokens: i64,
    top_p: f64,
    thinking_level: String,
    response_mime_type: String,
    response_schema: Value,
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    steps: Option<Vec<Step>>,
}

#[derive(Deserialize, Debug)]
struct Step {
    #[serde(rename = "type")]
    step_type: String,
    content: Option<Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AuditResult {
    pub calories: i32,
    #[serde(rename = "recipeCuisine")]
    pub recipe_cuisine: String,
}

impl GeminiService {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { client, api_key }
    }

    /// Calls the Gemini Interactions API to audit a recipe and extract SEO metadata.
    pub async fn audit_recipe(&self, recipe: &Recipe) -> Result<AuditResult, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/interactions?key={}",
            self.api_key
        );

        let prompt = format!(
            "Audit this recipe and estimate:
1. Calories (an integer representing estimated calories per serving).
2. Recipe cuisine (must be selected from the allowed enum list).

Recipe Details:
Title: {}
Description: {}
Categories: {}
Ingredients:
{}
Steps:
{}",
            recipe.title,
            recipe.description,
            recipe.categories.join(", "),
            recipe
                .ingredients
                .iter()
                .map(|i| format!("- {}", i))
                .collect::<Vec<_>>()
                .join("\n"),
            recipe
                .steps
                .iter()
                .enumerate()
                .map(|(idx, s)| format!("{}. {}", idx + 1, s))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let response_schema = json!({
            "type": "object",
            "properties": {
                "calories": { "type": "integer" },
                "recipeCuisine": {
                    "type": "string",
                    "enum": [
                        "Spanish",
                        "Catalan",
                        "Italian",
                        "Mexican",
                        "Japanese",
                        "Chinese",
                        "Indian",
                        "French",
                        "American",
                        "Mediterranean",
                        "Middle Eastern",
                        "Greek",
                        "Thai",
                        "Vietnamese",
                        "Moroccan",
                        "Turkish",
                        "Latin American",
                        "Caribbean",
                        "Nordic",
                        "British",
                        "German",
                        "Eastern European",
                        "African",
                        "Asian Fusion",
                        "International"
                    ]
                }
            },
            "propertyOrdering": ["calories", "recipeCuisine"],
            "required": ["calories", "recipeCuisine"]
        });

        let payload = GeminiRequest {
            model: "models/gemini-3.5-flash".to_string(),
            input: prompt,
            generation_config: GenerationConfig {
                temperature: 1.0,
                max_output_tokens: 65536,
                top_p: 0.95,
                thinking_level: "minimal".to_string(),
                response_mime_type: "application/json".to_string(),
                response_schema,
            },
        };

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("HTTP request error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(format!("Gemini API error ({}): {}", status, error_body));
        }

        let gemini_resp: GeminiResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Gemini response JSON: {}", e))?;

        let steps = gemini_resp
            .steps
            .ok_or_else(|| "No steps found in Gemini response".to_string())?;

        let text_content = extract_text_from_steps(&steps).ok_or_else(|| {
            "Could not find model output text in Gemini interaction steps".to_string()
        })?;

        let audit_result: AuditResult = serde_json::from_str(&text_content).map_err(|e| {
            format!(
                "Failed to deserialize model output JSON schema: {}. Raw text: {}",
                e, text_content
            )
        })?;

        Ok(audit_result)
    }
}

fn extract_text_from_steps(steps: &[Step]) -> Option<String> {
    for step in steps {
        if step.step_type == "model_output" {
            if let Some(content_val) = &step.content {
                if let Some(arr) = content_val.as_array() {
                    for part in arr {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            return Some(text.to_string());
                        }
                    }
                } else if let Some(text) = content_val.as_str() {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_text_from_steps_array() {
        let steps = vec![
            Step {
                step_type: "thought".to_string(),
                content: Some(json!("Thinking process...")),
            },
            Step {
                step_type: "model_output".to_string(),
                content: Some(json!([
                    {
                        "type": "text",
                        "text": "{\"calories\": 350, \"recipeCuisine\": \"Italian\"}"
                    }
                ])),
            },
        ];

        let text = extract_text_from_steps(&steps);
        assert_eq!(
            text,
            Some("{\"calories\": 350, \"recipeCuisine\": \"Italian\"}".to_string())
        );
    }

    #[test]
    fn test_extract_text_from_steps_string() {
        let steps = vec![Step {
            step_type: "model_output".to_string(),
            content: Some(json!("{\"calories\": 450, \"recipeCuisine\": \"Mexican\"}")),
        }];

        let text = extract_text_from_steps(&steps);
        assert_eq!(
            text,
            Some("{\"calories\": 450, \"recipeCuisine\": \"Mexican\"}".to_string())
        );
    }

    #[test]
    fn test_audit_result_deserialization() {
        let raw_json = "{\"calories\": 500, \"recipeCuisine\": \"Spanish\"}";
        let result: AuditResult = serde_json::from_str(raw_json).unwrap();
        assert_eq!(result.calories, 500);
        assert_eq!(result.recipe_cuisine, "Spanish");
    }
}
