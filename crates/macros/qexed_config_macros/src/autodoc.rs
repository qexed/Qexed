// qexed-config-macros/src/autodoc.rs
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr};

pub fn expand(input: DeriveInput) -> TokenStream {
    let struct_name = &input.ident;

    let fields = match &input.data {
        syn::Data::Struct(s) => &s.fields,
        _ => panic!("AutoDoc only supports structs"),
    };

    // 各个方法对应的代码生成器
    let mut doc_builders = Vec::new();
    let mut pending_builders = Vec::new();
    let mut warning_builders = Vec::new();
    let mut migration_builders = Vec::new();
    let mut deprecation_builders = Vec::new();
    let mut danger_builders = Vec::new();

    for field in fields.iter() {
        let ident = field.ident.as_ref().unwrap();
        let field_name = ident.to_string();
        let field_ty = &field.ty;

        // AutoDoc 属性值
        let mut key = None;
        let mut pending_deprecated = None;
        let mut warning = None;
        let mut migration_notice = None;
        let mut deprecation = None;
        let mut danger = None;
        let mut has_sub = false;

        // serde 序列化名称
        let mut serde_serialize_name: Option<String> = None;

        // 解析属性
        for attr in &field.attrs {
            // 处理 AutoDoc 属性
            if attr.path().is_ident("AutoDoc") {
                attr.parse_nested_meta(|meta| {
                    let name = meta.path.get_ident().unwrap().to_string();

                    match name.as_str() {
                        "key" => {
                            let value: LitStr = meta.value()?.parse()?;
                            key = Some(value.value());
                        }
                        "pending_deprecated" => {
                            let value: LitStr = meta.value()?.parse()?;
                            pending_deprecated = Some(value.value());
                        }
                        "warning" => {
                            let value: LitStr = meta.value()?.parse()?;
                            warning = Some(value.value());
                        }
                        "migration_notice" => {
                            let value: LitStr = meta.value()?.parse()?;
                            migration_notice = Some(value.value());
                        }
                        "deprecation" => {
                            let value: LitStr = meta.value()?.parse()?;
                            deprecation = Some(value.value());
                        }
                        "danger" => {
                            let value: LitStr = meta.value()?.parse()?;
                            danger = Some(value.value());
                        }
                        "sub" => {
                            // sub 没有参数值
                            has_sub = true;
                        }
                        _ => {}
                    }
                    Ok(())
                })
                .unwrap_or_else(|e| panic!("Failed to parse AutoDoc attribute: {}", e));
            }

            // 解析 serde(rename) 获取序列化名称
            if attr.path().is_ident("serde") {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        if let Ok(value) = meta.value() {
                            let lit: LitStr = value.parse()?;
                            serde_serialize_name = Some(lit.value());
                        } else {
                            meta.parse_nested_meta(|inner| {
                                if inner.path.is_ident("serialize") {
                                    let val: LitStr = inner.value()?.parse()?;
                                    serde_serialize_name = Some(val.value());
                                }
                                Ok(())
                            })?;
                        }
                    }
                    Ok(())
                });
            }
        }

        // 所有字段都必须有 AutoDoc key（无论是否有 sub）
        let key = key.unwrap_or_else(|| panic!("Field `{}` missing AutoDoc key", field_name));

        // 确定字段的显示名称（用于文档展示和递归前缀）
        let display_name = serde_serialize_name.unwrap_or(field_name);
        let display_name_lit = LitStr::new(&display_name, ident.span());

        // ---- 1. doc_fields ----
        // 字段本身的文档条目（始终有 key）
        doc_builders.push(quote! {
            all_doc.push((#display_name_lit.to_string(), ::rust_i18n::t!(#key, locale = lang).to_string()));
        });
        // 如果有 sub，递归子类型的 doc_fields，并拼接前缀
        if has_sub {
            doc_builders.push(quote! {
                for (sub_key, sub_desc) in <#field_ty as ::qexed_config_new::tool::AutoDocConfigTrait>::doc_fields(lang) {
                    all_doc.push((format!("{}.{}", #display_name_lit, sub_key), sub_desc));
                }
            });
        }

        // ---- 2. pending_deprecated_fields ----
        // 如果字段自身有 pending_deprecated 属性，添加自身条目
        if let Some(pd_key) = &pending_deprecated {
            pending_builders.push(quote! {
                all_pending.push((#display_name_lit.to_string(), ::rust_i18n::t!(#pd_key, locale = lang).to_string()));
            });
        }
        // 无论自身是否有 pending_deprecated，只要 has_sub，就需要递归子类型的 pending_deprecated
        if has_sub {
            pending_builders.push(quote! {
                for (sub_key, sub_desc) in <#field_ty as ::qexed_config_new::tool::AutoDocConfigTrait>::pending_deprecated_fields(lang) {
                    all_pending.push((format!("{}.{}", #display_name_lit, sub_key), sub_desc));
                }
            });
        }

        // ---- 3. warning_fields ----
        if let Some(warn_key) = &warning {
            warning_builders.push(quote! {
                all_warning.push((#display_name_lit.to_string(), ::rust_i18n::t!(#warn_key, locale = lang).to_string()));
            });
        }
        if has_sub {
            warning_builders.push(quote! {
                for (sub_key, sub_desc) in <#field_ty as ::qexed_config_new::tool::AutoDocConfigTrait>::warning_fields(lang) {
                    all_warning.push((format!("{}.{}", #display_name_lit, sub_key), sub_desc));
                }
            });
        }

        // ---- 4. migration_notice_fields ----
        if let Some(mig_key) = &migration_notice {
            migration_builders.push(quote! {
                all_migration.push((#display_name_lit.to_string(), ::rust_i18n::t!(#mig_key, locale = lang).to_string()));
            });
        }
        if has_sub {
            migration_builders.push(quote! {
                for (sub_key, sub_desc) in <#field_ty as ::qexed_config_new::tool::AutoDocConfigTrait>::migration_notice_fields(lang) {
                    all_migration.push((format!("{}.{}", #display_name_lit, sub_key), sub_desc));
                }
            });
        }

        // ---- 5. deprecation_fields ----
        if let Some(dep_key) = &deprecation {
            deprecation_builders.push(quote! {
                all_deprecation.push((#display_name_lit.to_string(), ::rust_i18n::t!(#dep_key, locale = lang).to_string()));
            });
        }
        if has_sub {
            deprecation_builders.push(quote! {
                for (sub_key, sub_desc) in <#field_ty as ::qexed_config_new::tool::AutoDocConfigTrait>::deprecation_fields(lang) {
                    all_deprecation.push((format!("{}.{}", #display_name_lit, sub_key), sub_desc));
                }
            });
        }

        // ---- 6. danger_fields ----
        if let Some(danger_key) = &danger {
            danger_builders.push(quote! {
                all_danger.push((#display_name_lit.to_string(), ::rust_i18n::t!(#danger_key, locale = lang).to_string()));
            });
        }
        if has_sub {
            danger_builders.push(quote! {
                for (sub_key, sub_desc) in <#field_ty as ::qexed_config_new::tool::AutoDocConfigTrait>::danger_fields(lang) {
                    all_danger.push((format!("{}.{}", #display_name_lit, sub_key), sub_desc));
                }
            });
        }
    }

    // 生成最终的 impl
    quote! {
        impl ::qexed_config_new::tool::AutoDocConfigTrait for #struct_name {
            fn doc_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_doc = Vec::new();
                #(#doc_builders)*
                all_doc
            }

            fn pending_deprecated_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_pending = Vec::new();
                #(#pending_builders)*
                all_pending
            }

            fn warning_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_warning = Vec::new();
                #(#warning_builders)*
                all_warning
            }

            fn migration_notice_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_migration = Vec::new();
                #(#migration_builders)*
                all_migration
            }

            fn deprecation_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_deprecation = Vec::new();
                #(#deprecation_builders)*
                all_deprecation
            }

            fn danger_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_danger = Vec::new();
                #(#danger_builders)*
                all_danger
            }
        }
    }
}