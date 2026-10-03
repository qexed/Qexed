use serde::Serialize;
use toml_edit::DocumentMut;
use crate::TomlError; // 加上 Item、Value
// toml_edit的to_document的封装，没啥特殊含义
pub fn to_document<T>(value: &T) -> Result<DocumentMut, TomlError>
where
    T: Serialize + ?Sized,
{
    Ok(toml_edit::ser::to_document(value)?)
}
// DocumentMut转换为原对象
// 依旧复用toml_edit原方法
pub fn from_document<T>(value: DocumentMut) -> Result<T, TomlError>
where
    T: serde::de::DeserializeOwned,
{
    Ok(toml_edit::de::from_document(value)?)
}
