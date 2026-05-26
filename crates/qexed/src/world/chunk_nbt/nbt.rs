use std::collections::HashMap;

use qexed_nbt::Tag;

pub(super) fn compound(tag: &Tag) -> Option<&HashMap<String, Tag>> {
    match tag {
        Tag::Compound(compound) => Some(compound),
        _ => None,
    }
}

pub(super) fn list_items(tag: Option<&Tag>) -> Option<&[Tag]> {
    match tag {
        Some(Tag::List(_, items)) => Some(items),
        _ => None,
    }
}

pub(super) fn long_array(tag: &Tag) -> Option<&[i64]> {
    match tag {
        Tag::LongArray(values) => Some(values),
        _ => None,
    }
}

pub(super) fn byte_array(tag: &Tag) -> Option<&[i8]> {
    match tag {
        Tag::ByteArray(values) => Some(values),
        _ => None,
    }
}

pub(super) fn string_field<'a>(compound: &'a HashMap<String, Tag>, name: &str) -> Option<&'a str> {
    match compound.get(name) {
        Some(Tag::String(value)) => Some(value),
        _ => None,
    }
}

pub(super) fn int_field(compound: &HashMap<String, Tag>, name: &str) -> Option<i32> {
    match compound.get(name) {
        Some(Tag::Byte(value)) => Some(i32::from(*value)),
        Some(Tag::Short(value)) => Some(i32::from(*value)),
        Some(Tag::Int(value)) => Some(*value),
        Some(Tag::Long(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
}

pub(super) fn string_properties(value: Option<&Tag>) -> Vec<(String, String)> {
    let Some(properties) = value.and_then(compound) else {
        return Vec::new();
    };

    let mut properties = properties
        .iter()
        .filter_map(|(key, value)| match value {
            Tag::String(value) => Some((key.clone(), value.to_string())),
            _ => None,
        })
        .collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(&right.0));
    properties
}
