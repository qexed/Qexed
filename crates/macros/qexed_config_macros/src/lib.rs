//! app_config：代替手写 impl qexed_config::Config。
//!
//! 用法：
//! #[qexed_config_macros::app_config("/", "log")]
//!
//! 生成 `impl ::qexed_config::Config`（PATH/NAME/SECRETS，前导 `::` 强制解析到
//! extern prelude，调用方本地的同名模块/导入不会干扰），并把 <File>path/name.toml</File>
//! 与 <Secrets>...</Secrets> 注入类型的 autodoc 块（没有则补一个完整块）。
//! 机密规则只来自 autodoc 标签（<Secret /> 字段级、<Secrets> 结构体级），
//! 不接受 secrets 参数。

use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::parse::Parser;
use syn::{LitStr, Token};

#[proc_macro_attribute]
pub fn app_config(args: TokenStream, input: TokenStream) -> TokenStream {
    let arg_tokens: proc_macro2::TokenStream = args.into();
    let item_tokens: proc_macro2::TokenStream = input.into();
    match expand(arg_tokens, item_tokens) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}


fn expand(
    arg_tokens: proc_macro2::TokenStream,
    item_tokens: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let (config_path, config_name) = parse_args(arg_tokens)?;
    let item: syn::Item = syn::parse2(item_tokens)?;
    let item_struct = match &item {
        syn::Item::Struct(s) => s,
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "app_config only works on structs",
            ))
        }
    };
    let name = &item_struct.ident;

    // ---- 从 autodoc 收集机密模式（<Secret /> 字段标签 + <Secrets> 嵌套/通配路径）----
    // 属性宏先于 Derive 执行：读 doc 里的标签生成 SECRETS，并保持 doc 原样透传。
    let mut new_attrs: Vec<syn::Attribute> = Vec::new();
    for attr in &item_struct.attrs {
        new_attrs.push(attr.clone());
    }
    let mut secrets: Vec<String> = Vec::new();
    for attr in &item_struct.attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let text = attr_doc_text(attr);
        // <Secrets>：多行/逗号分隔的嵌套或通配模式，如 auth.password, servers.*.api_key
        if let Some(inner) = tag_inner(&text, "Secrets") {
            for part in inner.split([',', '\n']) {
                let pattern = part.trim();
                if !pattern.is_empty() && !secrets.iter().any(|s| s == pattern) {
                    secrets.push(pattern.to_string());
                }
            }
        }
    }
    // 字段级 <Secret />：裸字段名进模式表（serde rename 后的键名优先）。
    if let syn::Fields::Named(named) = &item_struct.fields {
        for field in named.named.iter() {
            let has_secret = field.attrs.iter().any(|attr| {
                attr.path().is_ident("doc") && tag_self_closing(&attr_doc_text(attr), "Secret")
            });
            if !has_secret {
                continue;
            }
            let rust_name = field.ident.as_ref().expect("named field").to_string();
            let key = serde_rename_of(field).unwrap_or(rust_name);
            if !secrets.iter().any(|s| s == &key) {
                secrets.push(key);
            }
        }
    }

    // ---- impl Config ----
    let path_lit = LitStr::new(&config_path, proc_macro2::Span::call_site());
    let name_lit = LitStr::new(&config_name, proc_macro2::Span::call_site());
    let secrets_lits = secrets.iter().map(|s| quote! { #s });

    // attrs 去重：new_attrs 已含注入结果，避免 item_struct 再带一份原 attrs
    let mut item_struct = item_struct.clone();
    item_struct.attrs = new_attrs;
    Ok(quote! {
        #item_struct

        impl ::qexed_config::Config for #name {
            const PATH: &'static str = #path_lit;
            const NAME: &'static str = #name_lit;
            const SECRETS: &'static [&'static str] = &[#(#secrets_lits),*];
        }
    })
}

fn attr_doc_text(attr: &syn::Attribute) -> String {
    if let syn::Meta::NameValue(nv) = &attr.meta {
        if let syn::Expr::Lit(lit) = &nv.value {
            if let syn::Lit::Str(s) = &lit.lit {
                return s.value();
            }
        }
    }
    String::new()
}

fn parse_args(tokens: proc_macro2::TokenStream) -> syn::Result<(String, String)> {
    let parser = Punctuated::<LitStr, Token![,]>::parse_terminated;
    let lits = parser.parse2(tokens)?;
    let values: Vec<String> = lits.into_iter().map(|l| l.value()).collect();
    match values.as_slice() {
        [path, name] => Ok((path.clone(), name.clone())),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "app_config(PATH, NAME): exactly two positional string arguments",
        )),
    }
}



/// 取标签内文（<Tag>...</Tag>），跨行有效。
fn tag_inner(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].to_string())
}

/// 自闭合标签判定（<Secret /> 及常见变体）。
fn tag_self_closing(text: &str, tag: &str) -> bool {
    text.contains(&format!("<{tag} />")) || text.contains(&format!("<{tag}/>"))
}

/// 字段的 serde rename 键名（显式 rename 才认；rename_all 交给文档侧的 serde 对齐逻辑）。
fn serde_rename_of(field: &syn::Field) -> Option<String> {
    for attr in &field.attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        if let Ok(nested) = attr.parse_args_with(
            Punctuated::<syn::Meta, Token![,]>::parse_terminated,
        ) {
            for meta in nested {
                if let syn::Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("rename") {
                        if let syn::Expr::Lit(lit) = &nv.value {
                            if let syn::Lit::Str(s) = &lit.lit {
                                return Some(s.value());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
