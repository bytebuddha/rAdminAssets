#[cfg(feature = "tera")]
pub mod tera {
    use radmin::serde_json::Value;
    use radmin::rocket_contrib::templates::tera::Result;

    use std::collections::HashMap;

    pub fn asset(value: Value, _: HashMap<String, Value>) -> Result<Value> {
        match value{
            Value::String(string) => {
                let path = std::path::PathBuf::from(&string);
                if let Some(Some(ext)) = path.extension().map(|item| item.to_str()) {
                    match &ext.to_lowercase()[..] {
                        "png" | "jpeg" | "jpg" | "svg" | "gif" => {
                            Ok(format!("/assets/img/{}", string).into())
                        },
                        ext => Ok(format!("/assets/{}/{}", ext, string).into())
                    }
                } else {
                    Ok(format!("/assets/{}", string).into())
                }
            },
            value => Ok(value),
        }
    }
}
