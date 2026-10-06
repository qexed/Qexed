use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(Doc)]
pub fn derive_doc(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) { 
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// 枚举值校验派生：为 enum 生成 qexed_doc::DocValue 实现。
/// 兼容 serde：容器 rename_all / 变体 rename / 变体 skip 都会影响
/// VARIANTS 与 try_from_name，保证校验的就是线上真实取值。
#[proc_macro_derive(DocValue)]
pub fn derive_doc_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_doc_value(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();
    // 结构体级块可选：被 <Attr name=sub /> 引用的类型不必有自己的块。
    let docs = doc_attrs(&input.attrs);
    let parsed = parse_doc(&docs).map_err(|e| syn::Error::new_spanned(name, e))?;
    let key = parsed.key.unwrap_or_default();
    // <Secrets> 机密字段模式（对应 impl Config 的 SECRETS）：
    // 一行一个或逗号分隔都支持，如 "token, auth.password, servers.*.api_key"。
    let mut secrets: Vec<String> = value_tag(&docs, "Secrets")
        .map(|text| {
            text.lines()
                .flat_map(|line| line.split(','))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // 结构体级可写：未声明 <Attr name="writable" /> 时默认只读。
    let writable = parsed.writable.unwrap_or(false);
    let serde_container = serde_container_attrs(&input.attrs)?;
    // 结构体级 #[serde(default)]：所有字段缺省都合法。
    let container_default = serde_container.default;

    // 结构体级 i18n 插值：所有 <Value name=x>expr</Value> 的执行结果。
    let schema_value_pairs = parsed
        .values
        .iter()
        .map(|(value_name, expr_text)| -> syn::Result<_> {
            let expr: syn::Expr = syn::parse_str(expr_text).map_err(|e| {
                syn::Error::new_spanned(name, format!("invalid <Value> expr: {e}"))
            })?;
    Ok(quote! { (#value_name.to_string(), qexed_doc::eval_default(&#expr)) })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => return Err(syn::Error::new_spanned(name, "Doc only supports named fields")),
        },
        _ => return Err(syn::Error::new_spanned(name, "Doc only supports structs")),
    };
    let mut field_tokens = Vec::new();
    let mut validate_tokens = Vec::new();
    let mut json_validate_tokens = Vec::new();
    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_docs = doc_attrs(&field.attrs);
        let field_doc = parse_doc(&field_docs).map_err(|e| {
            syn::Error::new_spanned(field_name, e)
        })?;
        let field_key = field_doc.key.ok_or_else(|| {
            syn::Error::new_spanned(field_name, "missing <Name> tag in autodoc block")
        })?;
        let ty = &field.ty;

        // ---- serde 字段属性：文档必须反映真实（反）序列化行为 ----
        let serde_field = serde_field_attrs(&field.attrs)?;
        if serde_field.skip || serde_field.skip_serializing || serde_field.skip_deserializing {
            // 不在配置里出现（或不接受配置值）的字段不进文档，避免文档撒谎。
            continue;
        }
        let serde_name = match &serde_field.rename {
            Some(explicit) => explicit.clone(),
            None => match &serde_container.rename_all {
                Some(rule) => apply_case(&field_name.to_string(), rule).map_err(|e| {
                    syn::Error::new_spanned(field_name, e)
                })?,
                None => field_name.to_string(),
            },
        };
        let is_flatten = serde_field.flatten;
        let is_sub = is_flatten || attr_flag(&field_docs, "sub");
        if is_flatten && option_inner(ty).is_some() {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[serde(flatten)] 不支持 Option 字段（serde 同样拒绝）",
            ));
        }
        let is_option = option_inner(ty).is_some();
        // 可写：字段级声明覆盖结构体级声明，未声明则继承。
        let writable = field_doc.writable.unwrap_or(writable);
        let path = serde_name.clone();

        // 字段级 i18n 插值对（表达式同样解析成 syn::Expr 再插值）。
        let value_pairs = field_doc
            .values
            .iter()
            .map(|(value_name, expr_text)| -> syn::Result<_> {
                let expr: syn::Expr = syn::parse_str(expr_text).map_err(|e| {
                    syn::Error::new_spanned(field_name, format!("invalid <Value> expr: {e}"))
                })?;
                Ok(quote! { (#value_name.to_string(), qexed_doc::eval_default(&#expr)) })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        // default 只表示字段默认值，与 i18n 翻译键无关：
        // 有 <Default> 用它（执行结果）；否则回退为第一个 <Value> 的执行结果。
        let default_expr = if let Some(expr_text) = &field_doc.default {
            // <Default> 是字段默认值字面量（裸文本如 Info 不必加引号）。
            quote! { Some(qexed_doc::default_from_literal(#expr_text)) }
        } else if let Some((_, expr_text)) = field_doc.values.first() {
            let expr: syn::Expr = syn::parse_str(expr_text).map_err(|e| {
                syn::Error::new_spanned(field_name, format!("invalid <Value> expr: {e}"))
            })?;
            quote! { Some(qexed_doc::eval_default(&#expr)) }
        } else if let Some(fn_path) = &serde_field.default_fn {
            // #[serde(default = "path")]：文档默认值 = 真实反序列化缺省值。
            let path_expr: syn::Expr = syn::parse_str(fn_path).map_err(|e| {
                syn::Error::new_spanned(field_name, format!("invalid serde default path: {e}"))
            })?;
            quote! { Some(qexed_doc::eval_default(&#path_expr())) }
        } else if serde_field.default || container_default {
            // #[serde(default)]：字段类型必须实现 Default（serde 的要求）。
            quote! { Some(qexed_doc::eval_default(&<#ty as ::std::default::Default>::default())) }
        } else if is_option {
            // Option 字段缺省即 None。
            quote! { Some(qexed_doc::serde_json::Value::Null) }
        } else {
            // turbofish：否则 json 校验里的 None.is_none() 推不出元素类型
            quote! { ::std::option::Option::<qexed_doc::serde_json::Value>::None }
        };

        // <Check> / <CheckErrorTip> 内文原样进 schema，文档站自行取用。
        let check_ts = field_doc
            .check
            .as_ref()
            .map(|text| quote! { Some(#text) })
            .unwrap_or_else(|| quote! { None });

        // 元数据类型：sub 用类型名；Option 用内层类型（校验也是对内层做的）。
        let meta_ty: &syn::Type = if is_option {
            option_inner(ty).expect("checked above")
        } else {
            ty
        };
        // <CheckErrorTip> 推断：枚举字段（VARIANTS 非空）没写 <Check> 也没写 tip 时，
        // 自动生成与默认校验消息一致的提示；用户写了一律保留用户的；
        // 非枚举（VARIANTS 空）不推 tip。
        let check_error_tip: syn::Expr = if is_sub {
            syn::parse_quote!(None)
        } else if field_doc.check.is_none()
            && field_doc.check_error_tip.is_none()
        {
            syn::parse_quote!({
                let variants = <#meta_ty as qexed_doc::DocValue>::VARIANTS;
                if variants.is_empty() {
                    None
                } else {
                    Some(format!("expected one of {}", variants.join(", ")))
                }
            })
        } else {
            field_doc
                .check_error_tip
                .as_ref()
                .map(|text| syn::parse_quote!(::std::option::Option::Some(#text.to_string())))
                .unwrap_or_else(|| syn::parse_quote!(None))
        };

        let value_type: syn::Expr = if is_sub {
            syn::parse_quote!(stringify!(#ty))
        } else {
            syn::parse_quote!(<#meta_ty as qexed_doc::DocValue>::type_label())
        };
        let variants: syn::Expr = if is_sub {
            syn::parse_quote!(&[] as &[&'static str])
        } else {
            syn::parse_quote!(<#meta_ty as qexed_doc::DocValue>::VARIANTS)
        };
        // alias 字面量：Vec<String> -> &'static [&'static str]
        let alias_lits = serde_field.aliases.iter().map(|a| quote! { #a });
        // <Select> 推断：字段类型是枚举（VARIANTS 非空）且用户没写 <Select> 时，
        // 自动用变体表做候选值；用户写了一律保留用户的。
        let select_ts = if !field_doc.select.is_empty() {
            let select_items = field_doc.select.iter().map(|v| quote! { #v });
            quote! { &[#(#select_items),*] }
        } else if is_sub {
            quote! { &[] as &[&'static str] }
        } else {
            quote! { <#meta_ty as qexed_doc::DocValue>::VARIANTS }
        };
        // <Min>/<Max> 推断：数值基本类型按位宽给全范围边界；用户写了保留用户的。
        let (min_expr, max_expr) = if field_doc.min.is_some() || field_doc.max.is_some() {
            (
                match field_doc.min {
                    Some(v) => quote! { Some(#v) },
                    None => quote! { None },
                },
                match field_doc.max {
                    Some(v) => quote! { Some(#v) },
                    None => quote! { None },
                },
            )
        } else if let Some((lo, hi)) = numeric_bounds_of(meta_ty) {
            (quote! { Some(#lo) }, quote! { Some(#hi) })
        } else {
            (quote! { None }, quote! { None })
        };
        // 机密判定：<IsPassword /> 显式声明，或字段路径命中 <Secrets> 模式
        //（模式支持 "field"、"parent.field"、"parent.*.child" 通配）。
        let is_secret = field_doc.is_password
            || field_doc.is_secret_tag
            || secrets.iter().any(|pattern| {
                pattern == &serde_name
                    || pattern == &field_name.to_string()
                    || secret_pattern_matches(pattern, &serde_name)
            });
        // 字段级 <Secret /> 命中：字段 serde 名补进模式表（schema.secrets 给前端并集用）
        if is_secret && !secrets.iter().any(|s| s == &serde_name) {
            secrets.push(serde_name.clone());
        }
        let password_lit = is_secret;

        // <Tip>/<Warn>：值原样传给运行时 classify_i18n_text 做键/文本分类。
        // 注意恒生成完整表达式（未声明时为 None），否则 with_tip() 会变成 0 参调用。
        let tip_expr = match field_doc.tip.as_deref() {
            Some(raw) => quote! { Some(qexed_doc::classify_i18n_text(#raw)) },
            None => quote! { None },
        };
        let warn_expr = match field_doc.warn.as_deref() {
            Some(raw) => quote! { Some(qexed_doc::classify_i18n_text(#raw)) },
            None => quote! { None },
        };

        let field_base = quote! {
            qexed_doc::doc_field(
                #path,
                #field_key,
                #writable,
                qexed_doc::doc_values(&[#(#value_pairs),*]),
                #default_expr,
                #value_type,
                #variants,
                #check_ts,
                #check_error_tip,
                &[#(#alias_lits),*],
                #password_lit,
            )
            .with_select(#select_ts)
            .with_min_max(#min_expr, #max_expr)
            .with_optional(#is_option)
            .with_flattened(#is_flatten)
            .with_tip(#tip_expr)
            .with_warn(#warn_expr)
        };
        if is_sub {
            field_tokens.push(quote! {
                #field_base.with_sub(<#ty as qexed_doc::DocSchemaOf>::schema())
            });
        } else {
            field_tokens.push(field_base);
        }

        // ---- 校验：嵌套/拍平委托子类型；Option 允许 None；缺省回退规则与 serde 一致 ----
        // serde 别名同样接受配置值：先正式名，再依次别名。
        let alias_fallback = serde_field
            .aliases
            .iter()
            .map(|alias| quote! { .or_else(|| value.get(#alias)) });
        let validate_ty: &syn::Type = if is_option {
            option_inner(ty).expect("checked above")
        } else {
            ty
        };
        // 缺省合法性：有 autodoc/serde default、Option、或机密字段
        //（机密字段在主文件可缺省，加载时从 .secrets 回填）。
        let missing_ok = serde_field.default
            || serde_field.default_fn.is_some()
            || container_default
            || is_option
            || is_secret;

        if is_flatten {
            // 拍平：子字段与父字段同级。结构体校验不带前缀委托；
            // JSON 校验把整个对象交给子类型（子类型只查自己的键）。
            validate_tokens.push(quote! {
                {
                    if let Err(message) = <#ty as qexed_doc::DocValidate>::validate(&self.#field_name) {
                        errors.push(message);
                    }
                }
            });
            json_validate_tokens.push(quote! {
                {
                    if let Err(message) = <#ty>::validate_json_value(value) {
                        errors.push(message);
                    }
                }
            });
        } else if is_sub {
            validate_tokens.push(quote! {
                {
                    let v = &self.#field_name;
                    if let Err(message) = <#ty as qexed_doc::DocValidate>::validate(v) {
                        errors.push(format!("{}: {}", #serde_name, message));
                    }
                }
            });
            json_validate_tokens.push(quote! {
                {
                    match value.get(#serde_name) #(#alias_fallback)* {
                        // <stored in .secrets> 占位符不是真实值：跳过值校验
                        Some(v) if qexed_doc::is_secret_placeholder(v) => {}
                        Some(v) => {
                            if let Err(message) = <#ty>::validate_json_value(v) {
                                errors.push(format!("{}: {}", #serde_name, message));
                            }
                        }
                        None => {
                            if !(#default_expr.is_some() || #missing_ok) {
                                errors.push(format!("{}: missing required field", #serde_name));
                            }
                        }
                    }
                }
            });
        } else if is_option {
            // 类型化校验：None 跳过，Some 校验内层。
            validate_tokens.push(quote! {
                {
                    if let Some(v) = &self.#field_name {
                        if let Err(message) = <#validate_ty as qexed_doc::DocValue>::validate(
                            &qexed_doc::serde_json::to_value(v).expect("serialize field"),
                        ) {
                            errors.push(format!("{}: {}", #serde_name, message));
                        }
                    }
                }
            });
            json_validate_tokens.push(quote! {
                {
                    match value.get(#serde_name) #(#alias_fallback)* {
                        // null = None，合法
                        Some(v) if v.is_null() => {}
                        Some(v) => {
                            if let Err(message) = <#validate_ty as qexed_doc::DocValue>::validate(v) {
                                errors.push(format!("{}: {}", #serde_name, message));
                            }
                        }
                        None => {
                            if !(#default_expr.is_some() || #missing_ok) {
                                errors.push(format!("{}: missing required field", #serde_name));
                            }
                        }
                    }
                }
            });
        } else {
            validate_tokens.push(quote! {
                {
                    let v = &self.#field_name;
                    if let Err(message) = <#ty as qexed_doc::DocValue>::validate(
                        &qexed_doc::serde_json::to_value(v).expect("serialize field"),
                    ) {
                        errors.push(format!("{}: {}", #serde_name, message));
                    }
                }
            });
            // 值校验补全链：DocValue（类型/变体）→ <Select> → <Min>/<Max>。
            json_validate_tokens.push(quote! {
                {
                    match value.get(#serde_name) #(#alias_fallback)* {
                        // <stored in .secrets> 占位符不是真实值：跳过值校验
                        Some(v) if qexed_doc::is_secret_placeholder(v) => {}
                        Some(v) => {
                            if let Err(message) = <#ty as qexed_doc::DocValue>::validate(v) {
                                errors.push(format!("{}: {}", #serde_name, message));
                            }
                            qexed_doc::validate_doc_hints(
                                v,
                                #select_ts,
                                #min_expr,
                                #max_expr,
                            ).iter().for_each(|m| errors.push(format!("{}: {}", #serde_name, m)));
                        }
                        None => {
                            if !(#default_expr.is_some() || #missing_ok) {
                                errors.push(format!("{}: missing required field", #serde_name));
                            }
                        }
                    }
                }
            });
        }
    }
    let secrets_lits = secrets.iter().map(|s| quote! { #s });
    let secrets_empty = secrets.is_empty();
    Ok(quote! {
        impl #name {
            pub fn doc_fields() -> Vec<qexed_doc::DocField> { vec![#(#field_tokens),*] }
            pub fn schema() -> qexed_doc::DocSchema {
                qexed_doc::DocSchema {
                    name: #name_str.to_string(),
                    key: #key.to_string(),
                    secrets: if #secrets_empty {
                        ::std::vec::Vec::new()
                    } else {
                        ::std::vec![#(#secrets_lits.to_string()),*]
                    },
                    secret_placeholder: qexed_doc::SECRET_PLACEHOLDER.to_string(),
                    values: qexed_doc::doc_values(&[#(#schema_value_pairs),*]),
                    fields: Self::doc_fields(),
                    // 空占位：由 qexed_language::translate_schema 在渲染阶段回填。
                    document: ::std::option::Option::None,
                }
            }
            pub fn schema_json() -> String { Self::schema().to_json() }
            pub fn validate_struct(&self) -> Result<(), String> {
                qexed_doc::DocValidate::validate(self)
            }
            pub fn validate_json(json: &str) -> Result<(), String> {
                let value: qexed_doc::serde_json::Value =
                    qexed_doc::serde_json::from_str(json)
                        .map_err(|e| format!("invalid json: {e}"))?;
                Self::validate_json_value(&value)
            }
            /// 校验一个已解析的 JSON 对象；嵌套字段递归调用子类型的本方法。
            pub fn validate_json_value(value: &qexed_doc::serde_json::Value) -> Result<(), String> {
                if !value.is_object() {
                    return Err("expected object".to_string());
                }
                let mut errors: Vec<String> = Vec::new();
                #( #json_validate_tokens )*
                if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
            }
        }

        impl qexed_doc::DocValidate for #name {
            fn validate(&self) -> Result<(), String> {
                let mut errors: Vec<String> = Vec::new();
                #( #validate_tokens )*
                if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
            }
        }

        impl qexed_doc::DocSchemaOf for #name {
            fn schema() -> qexed_doc::DocSchema { Self::schema() }
        }
    })
}

fn expand_doc_value(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => return Err(syn::Error::new_spanned(name, "DocValue only supports enums")),
    };
    let container = serde_container_attrs(&input.attrs)?;
    let mut names: Vec<String> = Vec::new();
    for variant in variants {
        match &variant.fields {
            Fields::Unit => {}
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "DocValue only supports unit variants",
                ))
            }
        }
        // #[serde(skip)] 的变体不参与序列化，也不进文档取值表。
        let mut skipped = false;
        let mut explicit_rename: Option<String> = None;
        for attr in variant.attrs.iter().filter(|a| a.path().is_ident("serde")) {
            let nested = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )?;
            for meta in nested {
                match &meta {
                    syn::Meta::Path(p) if p.is_ident("skip") => skipped = true,
                    syn::Meta::NameValue(nv) if nv.path.is_ident("rename") => {
                        if let syn::Expr::Lit(lit) = &nv.value {
                            if let syn::Lit::Str(s) = &lit.lit {
                                explicit_rename = Some(s.value());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if skipped {
            continue;
        }
        let serde_name = match explicit_rename {
            Some(renamed) => renamed,
            None => match &container.rename_all {
                Some(rule) => {
                    apply_case(&variant.ident.to_string(), rule)
                        .map_err(|e| syn::Error::new_spanned(variant, e))?
                }
                None => variant.ident.to_string(),
            },
        };
        names.push(serde_name);
    }
    // 注意：匹配必须用字符串字面量模式；裸标识符模式会绑定任意输入。
    let match_idents = variants.iter().zip(names.iter()).map(|(variant, serde_name)| {
        let lit = syn::LitStr::new(serde_name, proc_macro2::Span::call_site());
        let ident = &variant.ident;
        quote! { #lit => Some(Self::#ident) }
    });
    Ok(quote! {
        impl qexed_doc::DocValue for #name {
            const TYPE: &'static str = stringify!(#name);
            const VARIANTS: &'static [&'static str] = &[#(#names),*];
            fn validate(value: &qexed_doc::serde_json::Value) -> Result<(), String> {
                let Some(name) = value.as_str() else {
                    return Err(format!("expected one of {}", Self::VARIANTS.join(", ")));
                };
                match Self::try_from_name(name) {
                    Some(_) => Ok(()),
                    None => Err(format!("expected one of {}", Self::VARIANTS.join(", "))),
                }
            }
        }

        impl #name {
            pub fn try_from_name(name: &str) -> Option<Self> {
                match name {
                    #( #match_idents, )*
                    _ => None,
                }
            }
        }
    })
}

// ---------------------------------------------------------------------------
// serde 属性解析
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SerdeContainer {
    rename_all: Option<String>,
    default: bool,
}

fn serde_container_attrs(attrs: &[syn::Attribute]) -> syn::Result<SerdeContainer> {
    let mut out = SerdeContainer::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let nested = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        )?;
        for meta in nested {
            match &meta {
                syn::Meta::NameValue(nv) if nv.path.is_ident("rename_all") => {
                    if let syn::Expr::Lit(lit) = &nv.value {
                        if let syn::Lit::Str(s) = &lit.lit {
                            out.rename_all = Some(s.value());
                        }
                    }
                }
                syn::Meta::Path(p) if p.is_ident("default") => out.default = true,
                _ => {}
            }
        }
    }
    Ok(out)
}

#[derive(Default)]
struct SerdeField {
    rename: Option<String>,
    aliases: Vec<String>,
    skip: bool,
    skip_serializing: bool,
    skip_deserializing: bool,
    flatten: bool,
    /// #[serde(default = "path")] 的函数路径。
    default_fn: Option<String>,
    /// #[serde(default)]（裸形式）。
    default: bool,
}

fn serde_field_attrs(attrs: &[syn::Attribute]) -> syn::Result<SerdeField> {
    let mut out = SerdeField::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let nested = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        )?;
        for meta in nested {
            match &meta {
                syn::Meta::Path(p) => {
                    if p.is_ident("skip") { out.skip = true; }
                    else if p.is_ident("flatten") { out.flatten = true; }
                    else if p.is_ident("default") { out.default = true; }
                    else if p.is_ident("skip_serializing") { out.skip_serializing = true; }
                    else if p.is_ident("skip_deserializing") { out.skip_deserializing = true; }
                }
                syn::Meta::NameValue(nv) => {
                    let value = |nv: &syn::MetaNameValue| -> Option<String> {
                        if let syn::Expr::Lit(lit) = &nv.value {
                            if let syn::Lit::Str(s) = &lit.lit {
                                return Some(s.value());
                            }
                        }
                        None
                    };
                    if nv.path.is_ident("rename") {
                        out.rename = value(nv);
                    } else if nv.path.is_ident("alias") {
                        if let Some(a) = value(nv) { out.aliases.push(a); }
                    } else if nv.path.is_ident("default") {
                        out.default_fn = value(nv);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

/// Type 是否为 Option<T>；是则返回 T。
fn option_inner(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(tp) = ty else { return None };
    let segment = tp.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else { return None };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner)) => Some(inner),
        _ => None,
    }
}


/// 数值基本类型的全范围边界（含端点）；非整数类型返回 None（浮点不推边界）。
fn numeric_bounds_of(ty: &syn::Type) -> Option<(f64, f64)> {
    let syn::Type::Path(path) = ty else { return None };
    let last = path.path.segments.last()?;
    let ident = last.ident.to_string();
    let bounds: (f64, f64) = match ident.as_str() {
        "u8" => (0.0, u8::MAX as f64),
        "u16" => (0.0, u16::MAX as f64),
        "u32" => (0.0, u32::MAX as f64),
        "u64" => (0.0, u64::MAX as f64),
        "usize" => (0.0, usize::MAX as f64),
        "i8" => (i8::MIN as f64, i8::MAX as f64),
        "i16" => (i16::MIN as f64, i16::MAX as f64),
        "i32" => (i32::MIN as f64, i32::MAX as f64),
        "i64" => (i64::MIN as f64, i64::MAX as f64),
        "isize" => (isize::MIN as f64, isize::MAX as f64),
        _ => return None,
    };
    Some(bounds)
}

/// SECRETS 模式匹配：`a.b.c` 精确前缀路径，`*` 通配单层，如 `servers.*.api_key`。
fn secret_pattern_matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('.').collect();
    let seg: Vec<&str> = path.split('.').collect();
    if pat.len() != seg.len() {
        return false;
    }
    pat.iter()
        .zip(seg.iter())
        .all(|(p, s)| *p == "*" || p == s)
}

/// serde rename_all 大小写规则。输入是 Rust 蛇形命名字段名或帕斯卡变体名。
fn apply_case(ident: &str, rule: &str) -> Result<String, String> {
    let words = |s: &str| -> Vec<String> {
        let mut out = Vec::new();
        for part in s.split('_').filter(|p| !p.is_empty()) {
            // 再按驼峰切分：Trace2Http -> Trace / Http
            let mut cur = String::new();
            let chars: Vec<char> = part.chars().collect();
            for (i, ch) in chars.iter().enumerate() {
                if ch.is_uppercase()
                    && i > 0
                    && (!chars[i - 1].is_uppercase() || chars.get(i + 1).map_or(false, |n| n.is_lowercase()))
                {
                    out.push(cur.clone());
                    cur.clear();
                }
                cur.push(*ch);
            }
            if !cur.is_empty() {
                out.push(cur);
            }
        }
        out
    };
    let ws = words(ident);
    match rule {
        "lowercase" => Ok(ident.to_lowercase()),
        "UPPERCASE" => Ok(ident.to_uppercase()),
        "snake_case" => Ok(ws.iter().map(|w| w.to_lowercase())
            .collect::<Vec<_>>().join("_")),
        "SCREAMING_SNAKE_CASE" => Ok(ws.iter().map(|w| w.to_uppercase())
            .collect::<Vec<_>>().join("_")),
        "kebab-case" => Ok(ws.iter().map(|w| w.to_lowercase())
            .collect::<Vec<_>>().join("-")),
        "camelCase" => Ok(ws.iter().enumerate().map(|(i, w)| {
            let lower = w.to_lowercase();
            if i == 0 { lower }
            else { capital(&lower) }
        }).collect::<Vec<_>>().join("")),
        "PascalCase" => Ok(ws.iter().map(|w| capital(&w.to_lowercase()))
            .collect::<Vec<_>>().join("")),
        other => Err(format!("不支持的 serde rename_all 规则: {other}")),
    }
}

fn capital(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// autodoc 块解析
// ---------------------------------------------------------------------------

fn doc_attrs(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(value) = &attr.meta else { return None; };
            let syn::Expr::Lit(expr) = &value.value else { return None; };
            let syn::Lit::Str(text) = &expr.lit else { return None; };
            Some(text.value())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Default)]
struct AutoDoc {
    key: Option<String>,
    /// (name, rust 表达式原文)
    values: Vec<(String, String)>,
    default: Option<String>,
    /// TypeScript 匿名校验函数原文（<Check> 内文）。
    check: Option<String>,
    /// 校验失败提示（<CheckErrorTip> 内文），替代默认错误消息。
    check_error_tip: Option<String>,
    /// <Select> 候选值列表：文档站下拉框；无 Check/变体时兼任值校验。
    select: Vec<String>,
    /// <Min>/<Max> 数值边界（含端点）。
    min: Option<f64>,
    max: Option<f64>,
    /// <IsPassword />：机密字段，文档站脱敏。
    is_password: bool,
    /// <Secret />：机密字段（qexed_config SECRETS 成员）。
    is_secret_tag: bool,
    /// None = 未声明；字段未声明时继承结构体级声明。
    writable: Option<bool>,
    /// <Tip>：字段补充说明（i18n 键或直接文本，原文，分类在运行时 helper 里做）。
    tip: Option<String>,
    /// <Warn>：字段警告（i18n 键或直接文本）。
    warn: Option<String>,
}

fn is_fence(line: &str) -> bool {
    line.chars().take_while(|ch| *ch == '\u{60}').count() >= 3
}

fn fence_info(line: &str) -> &str {
    line.trim_start_matches('\u{60}').trim()
}

fn parse_doc(docs: &str) -> Result<AutoDoc, String> {
    let mut block: Option<Vec<String>> = None;
    let mut closed = false;
    for line in docs.lines() {
        let trimmed = line.trim();
        if block.is_none() {
            if is_fence(trimmed) && fence_info(trimmed) == "autodoc" {
                block = Some(Vec::new());
            }
            continue;
        }
        if is_fence(trimmed) {
            closed = true;
            break;
        }
        if let Some(lines) = &mut block {
            lines.push(trimmed.to_string());
        }
    }
    if !closed {
        return Ok(AutoDoc::default());
    }
    let body = strip_comments(&block.unwrap_or_default().join("\n"));
    // 属性值规范：普通字符串属性值必须带引号，裸值编译期报错
    check_attr_values(&body)?;
    Ok(AutoDoc {
        key: tag_text(&body, "Name"),
        values: parse_values(&body),
        default: value_tag(&body, "Default"),
        check: value_tag(&body, "Check"),
        check_error_tip: value_tag(&body, "CheckErrorTip"),
        select: parse_select(&body),
        min: value_tag(&body, "Min").and_then(|s| s.parse().ok()),
        max: value_tag(&body, "Max").and_then(|s| s.parse().ok()),
        is_password: attr_flag(&body, "IsPassword"),
        // <Secret /> 自闭合标签（attr_flag 只认 <Attr name=...>，不适用）
        is_secret_tag: {
            let mut found = false;
            let mut rest = &body[..];
            while let Some(pos) = rest.find("<Secret") {
                let after = &rest[pos + "<Secret".len()..];
                if let Some(end) = after.find("/>") {
                    found = true;
                    rest = &after[end + 2..];
                } else {
                    break;
                }
            }
            found
        },
        writable: if attr_flag(&body, "writable") { Some(true) } else { None },
        tip: value_tag(&body, "Tip"),
        warn: value_tag(&body, "Warn"),
    })
}

/// 去掉 XML 注释 <!-- ... -->（可跨行），避免注释里的字样干扰标签解析。
fn strip_comments(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        match after.find("-->") {
            Some(end) => rest = &after[end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// 解析 <Select> 内的全部 <Select.Value>x</Select.Value>。
fn parse_select(block: &str) -> Vec<String> {
    let inner = value_tag(block, "Select").unwrap_or_default();
    let mut out = Vec::new();
    let mut rest = &inner[..];
    while let Some(pos) = rest.find("<Select.Value>") {
        let after = &rest[pos + "<Select.Value>".len()..];
        let Some(end) = after.find("</Select.Value>") else { break };
        out.push(after[..end].trim().to_string());
        rest = &after[end + "</Select.Value>".len()..];
    }
    out
}

/// 解析全部 <Value name=x>expr</Value> 标签。
fn parse_values(block: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = block;
    while let Some(pos) = rest.find("<Value") {
        let after = &rest[pos..];
        let Some(open_end) = after.find('>') else { break };
        let open_tag = &after[..open_end];
        let Some(value_name) = attr_value(open_tag, "name").unwrap_or(None) else { break };
        let body_start = &after[open_end + 1..];
        let Some(close) = body_start.find("</Value>") else { break };
        out.push((value_name, body_start[..close].trim().to_string()));
        rest = &body_start[close + "</Value>".len()..];
    }
    out
}

/// 解析 <Default>...</Default>（仅默认值，与翻译键无关）等无属性标签内文。
fn value_tag(block: &str, tag: &str) -> Option<String> {
    let start = block.find(&format!("<{tag}"))?;
    let open_end = block[start..].find('>')? + start + 1;
    let close = block[open_end..].find(&format!("</{tag}>"))? + open_end;
    Some(block[open_end..close].trim().to_string())
}

fn tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(block[start..end].trim().to_string())
}

/// 从开标签文本中取属性值。规范：普通字符串属性值必须带引号
/// （name="system" 或 name='system'）；裸值 name=system 视为错误。
/// 无该属性返回 Ok(None)，裸值返回 Err（编译期报错，避免静默失效）。
fn attr_value(tag: &str, attr: &str) -> Result<Option<String>, String> {
    let pattern = format!("{attr}=");
    let Some(idx) = tag.find(&pattern) else { return Ok(None) };
    let rest = &tag[idx + pattern.len()..];
    let first = rest.chars().next();
    let quotes = ['"', '\''];
    match first {
        // 带引号：取到配对闭引号（标签尾部 > 或 /> 不参与值）
        Some(q) if quotes.contains(&q) => {
            let body = &rest[1..];
            match body.find(q) {
                Some(end) => Ok(Some(body[..end].to_string())),
                None => Err(format!(
                    "属性 {attr} 的引号不配对：<xxx {attr}={rest}>"
                )),
            }
        }
        // 裸值：普通字符串必须带引号
        Some(_) => {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let bare = rest[..end].trim_end_matches('>').trim_end_matches('/');
            Err(format!(
                "属性 {attr} 的值必须是带引号的字符串：{attr}=\"{bare}\"（写法 <xxx {attr}={bare}> 不符合规范）"
            ))
        }
        None => Err(format!("属性 {attr} 缺少值：<xxx {attr}=>")),
    }
}

/// 属性值规范检查：块内所有 <Attr ...> / <Value ...> 的字符串属性值必须带引号。
fn check_attr_values(block: &str) -> Result<(), String> {
    let mut rest = block;
    while let Some(pos) = rest.find('<') {
        let after = &rest[pos..];
        let Some(open_end) = after.find('>') else { break };
        let tag = &after[..open_end + 1];
        if tag.starts_with("<Attr") || tag.starts_with("<Value") {
            // name 是当前唯二的属性；attr_value 内部强制引号规范
            attr_value(tag, "name")?;
        }
        rest = &after[1..];
    }
    Ok(())
}

/// 解析 <Attr name="writable" /> 形式的开关属性（裸值已由 check_attr_values 报错，
/// 此处对解析不到的按 false 处理）。
fn attr_flag(block: &str, flag: &str) -> bool {
    let mut rest = block;
    while let Some(pos) = rest.find("<Attr") {
        let after = &rest[pos..];
        let Some(end) = after.find("/>") else { break };
        if let Ok(Some(value)) = attr_value(&after[..end + 2], "name") {
            if value == flag {
                return true;
            }
        }
        rest = &after[end..];
    }
    false
}