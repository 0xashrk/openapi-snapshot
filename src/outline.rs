use serde_json::{json, Value};

use crate::errors::AppError;

pub fn outline_openapi(value: &Value) -> Result<Value, AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Outline("OpenAPI document must be a JSON object".to_string()))?;

    let paths = object
        .get("paths")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::Outline("OpenAPI document missing paths".to_string()))?;
    let schemas = object
        .get("components")
        .and_then(|v| v.as_object())
        .and_then(|components| components.get("schemas"))
        .and_then(|v| v.as_object());

    let outlined_paths = outline_paths(paths)?;
    let outlined_schemas = outline_schemas(schemas);

    Ok(json!({
        "paths": outlined_paths,
        "schemas": outlined_schemas,
    }))
}

fn outline_paths(paths: &serde_json::Map<String, Value>) -> Result<Value, AppError> {
    let mut outlined = serde_json::Map::new();
    for (path, item) in paths {
        let item_obj = item.as_object().ok_or_else(|| {
            AppError::Outline(format!("path item must be an object: {path}"))
        })?;
        let mut methods = serde_json::Map::new();
        for (method, op) in item_obj {
            if !is_http_method(method) {
                continue;
            }
            let op_obj = op.as_object().ok_or_else(|| {
                AppError::Outline(format!("operation must be an object: {path} {method}"))
            })?;
            let query = outline_query_params(op_obj);
            let request = outline_request_body(op_obj);
            let responses = outline_responses(op_obj);
            methods.insert(
                method.to_string(),
                json!({
                    "query": query,
                    "request": request,
                    "responses": responses,
                }),
            );
        }
        outlined.insert(path.to_string(), Value::Object(methods));
    }
    Ok(Value::Object(outlined))
}

fn is_http_method(method: &str) -> bool {
    matches!(
        method,
        "get" | "post" | "put" | "patch" | "delete" | "options" | "head" | "trace"
    )
}

fn outline_query_params(op: &serde_json::Map<String, Value>) -> Value {
    let params = op
        .get("parameters")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|param| outline_query_param(param).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Value::Array(params)
}

fn outline_query_param(param: &Value) -> Result<Value, AppError> {
    if let Some(reference) = param.get("$ref").and_then(|v| v.as_str()) {
        return Ok(json!({"$ref": reference}));
    }
    let obj = param
        .as_object()
        .ok_or_else(|| AppError::Outline("parameter must be an object".to_string()))?;
    let location = obj.get("in").and_then(|v| v.as_str()).unwrap_or("");
    if location != "query" {
        return Err(AppError::Outline("non-query parameter".to_string()));
    }
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return Err(AppError::Outline("query parameter missing name".to_string()));
    }
    let required = obj.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
    let schema = obj
        .get("schema")
        .map(schema_ref_or_type)
        .unwrap_or(Value::Null);

    Ok(json!({
        "name": name,
        "required": required,
        "schema": schema,
    }))
}

fn outline_request_body(op: &serde_json::Map<String, Value>) -> Value {
    let request_body = match op.get("requestBody") {
        Some(value) => value,
        None => return Value::Null,
    };
    if let Some(reference) = request_body.get("$ref").and_then(|v| v.as_str()) {
        return Value::String(reference.to_string());
    }
    let content = request_body
        .get("content")
        .and_then(|v| v.as_object())
        .and_then(select_content_schema);
    content.unwrap_or(Value::Null)
}

fn outline_responses(op: &serde_json::Map<String, Value>) -> Value {
    let responses = op
        .get("responses")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .map(|(code, response)| {
                    let schema = if let Some(reference) = response.get("$ref").and_then(|v| v.as_str())
                    {
                        Value::String(reference.to_string())
                    } else {
                        response
                            .get("content")
                            .and_then(|v| v.as_object())
                            .and_then(select_content_schema)
                            .unwrap_or(Value::Null)
                    };
                    (code.to_string(), schema)
                })
                .collect::<serde_json::Map<_, _>>()
        })
        .unwrap_or_default();
    Value::Object(responses)
}

fn select_content_schema(content: &serde_json::Map<String, Value>) -> Option<Value> {
    if let Some(schema) = content
        .get("application/json")
        .and_then(|v| v.get("schema"))
    {
        return Some(schema_ref_or_type(schema));
    }
    content
        .values()
        .filter_map(|entry| entry.get("schema"))
        .next()
        .map(schema_ref_or_type)
}

fn outline_schemas(schemas: Option<&serde_json::Map<String, Value>>) -> Value {
    let mut outlined = serde_json::Map::new();
    if let Some(schemas) = schemas {
        for (name, schema) in schemas {
            outlined.insert(name.to_string(), simplify_schema_definition(schema));
        }
    }
    Value::Object(outlined)
}

fn simplify_schema_definition(schema: &Value) -> Value {
    if let Some(reference) = schema.get("$ref").and_then(|v| v.as_str()) {
        return json!({"$ref": reference});
    }
    if let Some(of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        return json!({"oneOf": of.iter().map(schema_ref_or_type).collect::<Vec<_>>()});
    }
    if let Some(of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        return json!({"anyOf": of.iter().map(schema_ref_or_type).collect::<Vec<_>>()});
    }
    if let Some(of) = schema.get("allOf").and_then(|v| v.as_array()) {
        return json!({"allOf": of.iter().map(schema_ref_or_type).collect::<Vec<_>>()});
    }
    let schema_type = schema.get("type").and_then(|v| v.as_str());
    match schema_type {
        Some("object") | None => {
            let properties = schema
                .get("properties")
                .and_then(|v| v.as_object())
                .map(|props| {
                    props
                        .iter()
                        .map(|(name, value)| (name.to_string(), schema_ref_or_type(value)))
                        .collect::<serde_json::Map<_, _>>()
                });
            let required = schema
                .get("required")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                });
            let mut obj = serde_json::Map::new();
            obj.insert("type".to_string(), Value::String("object".to_string()));
            if let Some(required) = required {
                obj.insert(
                    "required".to_string(),
                    Value::Array(required.into_iter().map(Value::String).collect()),
                );
            }
            if let Some(properties) = properties {
                obj.insert("properties".to_string(), Value::Object(properties));
            }
            Value::Object(obj)
        }
        Some("array") => {
            let items = schema
                .get("items")
                .map(schema_ref_or_type)
                .unwrap_or(Value::Null);
            json!({"type": "array", "items": items})
        }
        Some(other) => json!({"type": other}),
    }
}

fn schema_ref_or_type(schema: &Value) -> Value {
    if let Some(reference) = schema.get("$ref").and_then(|v| v.as_str()) {
        return Value::String(reference.to_string());
    }
    if let Some(of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        return json!({"oneOf": of.iter().map(schema_ref_or_type).collect::<Vec<_>>()});
    }
    if let Some(of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        return json!({"anyOf": of.iter().map(schema_ref_or_type).collect::<Vec<_>>()});
    }
    if let Some(of) = schema.get("allOf").and_then(|v| v.as_array()) {
        return json!({"allOf": of.iter().map(schema_ref_or_type).collect::<Vec<_>>()});
    }
    let schema_type = schema.get("type").and_then(|v| v.as_str());
    match schema_type {
        Some("array") => {
            let items = schema
                .get("items")
                .map(schema_ref_or_type)
                .unwrap_or(Value::Null);
            json!({"type": "array", "items": items})
        }
        Some(other) => Value::String(other.to_string()),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_openapi_creates_minimal_shape() {
        let input = json!({
            "openapi": "3.0.3",
            "paths": {
                "/health": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/HealthResponse" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "HealthResponse": {
                        "type": "object",
                        "required": ["status"],
                        "properties": {
                            "status": { "type": "string" }
                        }
                    }
                }
            }
        });

        let output = outline_openapi(&input).unwrap();
        let responses = output["paths"]["/health"]["get"]["responses"]["200"]
            .as_str()
            .unwrap();
        assert_eq!(responses, "#/components/schemas/HealthResponse");

        let status = output["schemas"]["HealthResponse"]["properties"]["status"]
            .as_str()
            .unwrap();
        assert_eq!(status, "string");
    }
}
