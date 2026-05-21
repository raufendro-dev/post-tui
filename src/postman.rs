use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::models::{
    BodyConfig, Collection, Folder, Header, HttpMethod, QueryParam, RequestItem, RequestModel,
};

pub fn parse_collection(input: &str) -> Result<Collection> {
    let root: Value =
        serde_json::from_str(input).context("Postman collection is not valid JSON")?;
    let name = root
        .pointer("/info/name")
        .and_then(Value::as_str)
        .unwrap_or("Imported collection")
        .to_string();

    let items = root
        .get("item")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("This does not look like a Postman Collection v2.1 file."))?;

    let mut collection = Collection {
        name,
        ..Collection::default()
    };

    for item in items {
        parse_item(item, &mut collection.folders, &mut collection.requests)?;
    }

    Ok(collection)
}

fn parse_item(
    item: &Value,
    folders: &mut Vec<Folder>,
    requests: &mut Vec<RequestItem>,
) -> Result<()> {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Unnamed")
        .to_string();

    if item.get("request").is_some() {
        requests.push(RequestItem {
            name: name.clone(),
            request: parse_request(&name, item.get("request").unwrap_or(&Value::Null)),
        });
        return Ok(());
    }

    if let Some(children) = item.get("item").and_then(Value::as_array) {
        let mut folder = Folder {
            name,
            ..Folder::default()
        };
        for child in children {
            parse_item(child, &mut folder.folders, &mut folder.requests)?;
        }
        folders.push(folder);
    }

    Ok(())
}

fn parse_request(name: &str, value: &Value) -> RequestModel {
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .map(parse_method)
        .unwrap_or_default();

    let (url, query_params) = parse_url(value.get("url").unwrap_or(&Value::Null));
    let headers = value
        .get("header")
        .and_then(Value::as_array)
        .map(|headers| {
            headers
                .iter()
                .filter_map(|header| {
                    let key = header.get("key").and_then(Value::as_str)?.to_string();
                    let value = header
                        .get("value")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    Some(Header {
                        key,
                        value,
                        enabled: !header
                            .get("disabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    RequestModel {
        id: slug(name),
        name: name.to_string(),
        method,
        url,
        headers,
        query_params,
        auth: Default::default(),
        body: parse_body(value.get("body").unwrap_or(&Value::Null)),
    }
}

fn parse_method(method: &str) -> HttpMethod {
    match method.to_ascii_uppercase().as_str() {
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => HttpMethod::Get,
    }
}

fn parse_url(value: &Value) -> (String, Vec<QueryParam>) {
    if let Some(raw) = value.as_str() {
        return (raw.to_string(), vec![]);
    }

    if let Some(raw) = value.get("raw").and_then(Value::as_str) {
        return (raw.to_string(), parse_query(value));
    }

    let host = value
        .get("host")
        .and_then(Value::as_array)
        .map(|values| join_string_array(values))
        .unwrap_or_default();
    let path = value
        .get("path")
        .and_then(Value::as_array)
        .map(|values| join_path_array(values))
        .unwrap_or_default();
    let protocol = value
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("https");

    let url = if host.is_empty() {
        String::new()
    } else if path.is_empty() {
        format!("{protocol}://{host}")
    } else {
        format!("{protocol}://{host}/{path}")
    };

    (url, parse_query(value))
}

fn parse_query(value: &Value) -> Vec<QueryParam> {
    value
        .get("query")
        .and_then(Value::as_array)
        .map(|query| {
            query
                .iter()
                .filter_map(|param| {
                    let key = param.get("key").and_then(Value::as_str)?.to_string();
                    let value = param
                        .get("value")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    Some(QueryParam {
                        key,
                        value,
                        enabled: !param
                            .get("disabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_body(value: &Value) -> BodyConfig {
    match value.get("mode").and_then(Value::as_str) {
        Some("raw") => BodyConfig::Raw {
            content_type: "application/json".to_string(),
            body: value
                .get("raw")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        },
        _ => BodyConfig::Empty,
    }
}

fn join_string_array(values: &[Value]) -> String {
    values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(".")
}

fn join_path_array(values: &[Value]) -> String {
    values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("/")
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
