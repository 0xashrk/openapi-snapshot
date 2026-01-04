use serde_json::{json, Value};

use crate::errors::AppError;

type JsonMap = serde_json::Map<String, Value>;
type ResultValue = Result<Value, AppError>;

pub fn outline_openapi(value: &Value) -> ResultValue {
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
    let outlined_schemas = outline_schemas(schemas)?;

    Ok(json!({
        "paths": outlined_paths,
        "schemas": outlined_schemas,
    }))
}

fn outline_paths(paths: &JsonMap) -> ResultValue {
    let mut outlined = JsonMap::new();
    for (path, item) in paths {
        let item_obj = item
            .as_object()
            .ok_or_else(|| AppError::Outline(format!("path item must be an object: {path}")))?;

        let mut methods = JsonMap::new();
        for (method, op) in item_obj {
            if !is_http_method(method) {
                continue;
            }
            let op_obj = op.as_object().ok_or_else(|| {
                AppError::Outline(format!("operation must be an object: {path} {method}"))
            })?;
            let query = outline_query_params(op_obj)?;
            let request = outline_request_body(op_obj)?;
            let responses = outline_responses(op_obj)?;
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

fn outline_query_params(op: &JsonMap) -> ResultValue {
    let Some(raw_params) = op.get("parameters") else {
        return Ok(Value::Array(Vec::new()));
    };
    let params_array = raw_params
        .as_array()
        .ok_or_else(|| AppError::Outline("parameters must be an array".to_string()))?;

    let mut params = Vec::new();
    for param in params_array {
        params.push(outline_query_param(param)?);
    }
    Ok(Value::Array(params))
}

fn outline_query_param(param: &Value) -> ResultValue {
    if let Some(reference) = param.get("$ref").and_then(|v| v.as_str()) {
        return Ok(json!({"$ref": reference}));
    }

    let obj = param
        .as_object()
        .ok_or_else(|| AppError::Outline("parameter must be an object".to_string()))?;
    let location = obj
        .get("in")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Outline("parameter missing location".to_string()))?;
    if location != "query" {
        return Err(AppError::Outline("non-query parameter".to_string()));
    }

    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Outline("query parameter missing name".to_string()))?;
    if name.is_empty() {
        return Err(AppError::Outline(
            "query parameter missing name".to_string(),
        ));
    }

    let required = obj
        .get("required")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let schema_value = obj
        .get("schema")
        .ok_or_else(|| AppError::Outline("query parameter missing schema".to_string()))?;
    let schema = schema_ref_or_type(schema_value)?;

    Ok(json!({
        "name": name,
        "required": required,
        "schema": schema,
    }))
}

fn outline_request_body(op: &JsonMap) -> ResultValue {
    let Some(request_body) = op.get("requestBody") else {
        return Ok(Value::Null);
    };

    if let Some(reference) = request_body.get("$ref").and_then(|v| v.as_str()) {
        return Ok(Value::String(reference.to_string()));
    }

    let content = request_body
        .get("content")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::Outline("requestBody content must be an object".to_string()))?;

    select_content_schema(content)
}

fn outline_responses(op: &JsonMap) -> ResultValue {
    let responses = op
        .get("responses")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::Outline("responses must be an object".to_string()))?;

    let mut mapped = JsonMap::new();
    for (code, response) in responses {
        if let Some(reference) = response.get("$ref").and_then(|v| v.as_str()) {
            mapped.insert(code.to_string(), Value::String(reference.to_string()));
            continue;
        }

        let content = response
            .get("content")
            .and_then(|v| v.as_object())
            .ok_or_else(|| AppError::Outline(format!("response {code} missing content schema")))?;

        let schema = select_content_schema(content)?;
        mapped.insert(code.to_string(), schema);
    }

    Ok(Value::Object(mapped))
}

fn select_content_schema(content: &JsonMap) -> ResultValue {
    if let Some(schema) = content
        .get("application/json")
        .and_then(|v| v.get("schema"))
    {
        return schema_ref_or_type(schema);
    }

    for (_content_type, entry) in content {
        if let Some(schema) = entry.get("schema") {
            return schema_ref_or_type(schema);
        }
    }

    Err(AppError::Outline(
        "content missing schema for any content type".to_string(),
    ))
}

fn outline_schemas(schemas: Option<&JsonMap>) -> ResultValue {
    let mut outlined = JsonMap::new();
    if let Some(schemas) = schemas {
        for (name, schema) in schemas {
            outlined.insert(name.to_string(), simplify_schema_definition(schema)?);
        }
    }
    Ok(Value::Object(outlined))
}

fn simplify_schema_definition(schema: &Value) -> ResultValue {
    if let Some(reference) = schema.get("$ref").and_then(|v| v.as_str()) {
        return Ok(json!({"$ref": reference}));
    }

    if let Some(of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        return Ok(json!({"oneOf": collect_schema_vec(of)?}));
    }
    if let Some(of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        return Ok(json!({"anyOf": collect_schema_vec(of)?}));
    }
    if let Some(of) = schema.get("allOf").and_then(|v| v.as_array()) {
        return Ok(json!({"allOf": collect_schema_vec(of)?}));
    }

    let schema_type = schema.get("type").and_then(|v| v.as_str());
    match schema_type {
        Some("object") | None => {
            let properties = match schema.get("properties") {
                None => None,
                Some(Value::Object(props)) => {
                    let mut mapped = JsonMap::new();
                    for (name, value) in props {
                        mapped.insert(name.to_string(), schema_ref_or_type(value)?);
                    }
                    Some(mapped)
                }
                Some(_) => {
                    return Err(AppError::Outline(
                        "schema properties must be an object".to_string(),
                    ))
                }
            };

            let required = match schema.get("required") {
                None => None,
                Some(Value::Array(items)) => {
                    let mut names = Vec::new();
                    for item in items {
                        let Some(name) = item.as_str() else {
                            return Err(AppError::Outline(
                                "required entries must be strings".to_string(),
                            ));
                        };
                        names.push(name.to_string());
                    }
                    Some(names)
                }
                Some(_) => return Err(AppError::Outline("required must be an array".to_string())),
            };

            let mut obj = JsonMap::new();
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
            Ok(Value::Object(obj))
        }
        Some("array") => {
            let items = schema
                .get("items")
                .ok_or_else(|| AppError::Outline("array schema missing items".to_string()))?;
            Ok(json!({"type": "array", "items": schema_ref_or_type(items)?}))
        }
        Some(other) => Ok(Value::String(other.to_string())),
    }
}

fn collect_schema_vec(items: &[Value]) -> Result<Vec<Value>, AppError> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(schema_ref_or_type(item)?);
    }
    Ok(out)
}

fn schema_ref_or_type(schema: &Value) -> ResultValue {
    if let Some(reference) = schema.get("$ref").and_then(|v| v.as_str()) {
        return Ok(Value::String(reference.to_string()));
    }

    if let Some(of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        return Ok(json!({"oneOf": collect_schema_vec(of)?}));
    }
    if let Some(of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        return Ok(json!({"anyOf": collect_schema_vec(of)?}));
    }
    if let Some(of) = schema.get("allOf").and_then(|v| v.as_array()) {
        return Ok(json!({"allOf": collect_schema_vec(of)?}));
    }

    if let Some(schema_type) = schema.get("type").and_then(|v| v.as_str()) {
        match schema_type {
            "object" => simplify_schema_definition(schema),
            "array" => {
                let items = schema
                    .get("items")
                    .ok_or_else(|| AppError::Outline("array schema missing items".to_string()))?;
                Ok(json!({"type": "array", "items": schema_ref_or_type(items)?}))
            }
            other => Ok(Value::String(other.to_string())),
        }
    } else if schema.is_object() {
        simplify_schema_definition(schema)
    } else {
        Err(AppError::Outline("schema missing type".to_string()))
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

    #[test]
    fn outline_rejects_non_object_path_item() {
        let input = json!({
            "paths": {"/health": []},
            "components": {"schemas": {}}
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_non_query_parameter() {
        let input = json!({
            "paths": {
                "/health": {
                    "get": {
                        "parameters": [
                            {"in": "header", "name": "x", "schema": {"type": "string"}}
                        ],
                        "responses": {"200": {"content": {"application/json": {"schema": {"type": "string"}}}}}
                    }
                }
            },
            "components": {"schemas": {}}
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_missing_parameter_name() {
        let input = json!({
            "paths": {
                "/health": {
                    "get": {
                        "parameters": [
                            {"in": "query", "schema": {"type": "string"}}
                        ],
                        "responses": {"200": {"content": {"application/json": {"schema": {"type": "string"}}}}}
                    }
                }
            },
            "components": {"schemas": {}}
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_missing_parameter_schema() {
        let input = json!({
            "paths": {
                "/health": {
                    "get": {
                        "parameters": [
                            {"in": "query", "name": "status"}
                        ],
                        "responses": {"200": {"content": {"application/json": {"schema": {"type": "string"}}}}}
                    }
                }
            },
            "components": {"schemas": {}}
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_missing_content_schema() {
        let input = json!({
            "paths": {
                "/health": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {"application/json": {}}
                            }
                        }
                    }
                }
            }
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_request_body_content_not_object() {
        let input = json!({
            "paths": {
                "/health": {
                    "post": {
                        "requestBody": {
                            "content": []
                        },
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": {"type": "string"}
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {"schemas": {}}
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_request_body_missing_schema() {
        let input = json!({
            "paths": {
                "/health": {
                    "post": {
                        "requestBody": {
                            "content": {"application/json": {}}
                        },
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": {"type": "string"}
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {"schemas": {}}
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_array_without_items() {
        let input = json!({
            "paths": {
                "/health": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {"type": "array"}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_required_not_array() {
        let input = json!({
            "components": {
                "schemas": {
                    "Foo": {
                        "type": "object",
                        "required": "status",
                        "properties": {"status": {"type": "string"}}
                    }
                }
            },
            "paths": {"/health": {}},
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }

    #[test]
    fn outline_rejects_properties_not_object() {
        let input = json!({
            "components": {
                "schemas": {
                    "Foo": {
                        "type": "object",
                        "properties": []
                    }
                }
            },
            "paths": {"/health": {}},
        });
        let err = outline_openapi(&input).unwrap_err();
        assert!(matches!(err, AppError::Outline(_)));
    }
}
