use qexed_protocol::types::TextComponent;

/// 构造 `{"text": "..."}` 形式的文本组件。
pub fn text_component(text: impl Into<String>) -> TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

/// 构造 `{"translate": "...", "with": [...]}` 形式的可翻译文本组件。
pub fn translatable_component(key: impl Into<String>, with: Vec<TextComponent>) -> TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(key.into())),
    );
    if !with.is_empty() {
        map.insert(
            "with".to_string(),
            qexed_nbt::Tag::List(
                qexed_nbt::ListHeader {
                    tag_id: qexed_nbt::tag_id::COMPOUND,
                    length: with.len() as i32,
                },
                std::sync::Arc::from(with.into_boxed_slice()),
            ),
        );
    }
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}
