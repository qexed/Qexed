use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    DeriveInput, Field, GenericArgument, Ident, LitStr, PathArguments, Token, Type,
    spanned::Spanned,
};

#[derive(Default)]
struct AutoDocAttrs {
    key: Option<String>,
    pending_deprecated: Option<String>,
    warning: Option<String>,
    migration_notice: Option<String>,
    deprecation: Option<String>,
    danger: Option<String>,
    default_display: Option<String>,
    sensitive: bool,
    has_sub: bool,
}

#[derive(Default)]
struct SerdeAttrs {
    serialize_name: Option<String>,
    flatten: bool,
    skip: bool,
}

struct FieldDoc {
    field_ty: Type,
    display_name: String,
    auto_doc: AutoDocAttrs,
    serde: SerdeAttrs,
    span: Span,
}

pub fn expand(input: DeriveInput) -> TokenStream {
    let struct_name = input.ident;

    let fields = match input.data {
        syn::Data::Struct(s) => s.fields,
        _ => {
            return syn::Error::new(struct_name.span(), "AutoDoc only supports structs")
                .to_compile_error();
        }
    };

    let mut errors: Option<syn::Error> = None;
    let mut field_docs = Vec::new();

    for field in fields.iter() {
        match parse_field_doc(field) {
            Ok(doc) => field_docs.push(doc),
            Err(error) => push_error(&mut errors, error),
        }
    }

    if let Some(error) = errors {
        return error.to_compile_error();
    }

    let mut doc_builders = Vec::new();
    let mut value_type_builders = Vec::new();
    let mut default_display_builders = Vec::new();
    let mut sensitive_builders = Vec::new();
    let mut pending_builders = Vec::new();
    let mut warning_builders = Vec::new();
    let mut migration_builders = Vec::new();
    let mut deprecation_builders = Vec::new();
    let mut danger_builders = Vec::new();

    for field in &field_docs {
        append_field_builders(
            field,
            &mut doc_builders,
            &mut value_type_builders,
            &mut default_display_builders,
            &mut sensitive_builders,
            &mut pending_builders,
            &mut warning_builders,
            &mut migration_builders,
            &mut deprecation_builders,
            &mut danger_builders,
        );
    }

    quote! {
        impl ::qexed_config_autodoc::AutoDocConfigTrait for #struct_name {
            fn doc_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_doc = Vec::new();
                #(#doc_builders)*
                all_doc
            }

            fn field_value_types() -> Vec<(String, &'static str)> {
                let mut all_value_types = Vec::new();
                #(#value_type_builders)*
                all_value_types
            }

            fn default_display_fields(lang: &str) -> Vec<(String, String)> {
                let mut all_default_display = Vec::new();
                let _ = lang;
                #(#default_display_builders)*
                all_default_display
            }

            fn sensitive_fields() -> Vec<String> {
                let mut all_sensitive = Vec::new();
                #(#sensitive_builders)*
                all_sensitive
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

fn parse_field_doc(field: &Field) -> syn::Result<FieldDoc> {
    let ident = field
        .ident
        .as_ref()
        .ok_or_else(|| syn::Error::new(field.span(), "AutoDoc only supports named fields"))?;
    let field_name = ident.to_string();
    let field_ty = field.ty.clone();

    let mut auto_doc = AutoDocAttrs::default();
    let mut serde = SerdeAttrs::default();

    for attr in &field.attrs {
        if attr.path().is_ident("AutoDoc") {
            parse_autodoc_attr(attr, &mut auto_doc)?;
        }

        if attr.path().is_ident("serde") {
            parse_serde_attr(attr, &mut serde)?;
        }
    }

    if !serde.flatten && !serde.skip && auto_doc.key.is_none() {
        return Err(syn::Error::new(
            ident.span(),
            format!("Field `{}` missing AutoDoc key", field_name),
        ));
    }

    let display_name = serde
        .serialize_name
        .clone()
        .unwrap_or_else(|| field_name.clone());

    Ok(FieldDoc {
        field_ty,
        display_name,
        auto_doc,
        serde,
        span: ident.span(),
    })
}

fn parse_autodoc_attr(attr: &syn::Attribute, auto_doc: &mut AutoDocAttrs) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("key") {
            auto_doc.key = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("pending_deprecated") {
            auto_doc.pending_deprecated = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("warning") {
            auto_doc.warning = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("migration_notice") {
            auto_doc.migration_notice = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("deprecation") {
            auto_doc.deprecation = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("danger") {
            auto_doc.danger = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("default_display") {
            auto_doc.default_display = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("sensitive") {
            auto_doc.sensitive = true;
        } else if meta.path.is_ident("sub") {
            auto_doc.has_sub = true;
        }

        Ok(())
    })
}

fn parse_serde_attr(attr: &syn::Attribute, serde: &mut SerdeAttrs) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("flatten") {
            serde.flatten = true;
            return Ok(());
        }

        if meta.path.is_ident("skip") {
            serde.skip = true;
            return Ok(());
        }

        if meta.path.is_ident("rename") {
            if meta.input.peek(Token![=]) {
                let value = meta.value()?;
                serde.serialize_name = Some(value.parse::<LitStr>()?.value());
            } else {
                meta.parse_nested_meta(|inner| {
                    if inner.path.is_ident("serialize") {
                        serde.serialize_name = Some(inner.value()?.parse::<LitStr>()?.value());
                    }
                    Ok(())
                })?;
            }

            return Ok(());
        }

        if meta.input.peek(Token![=]) {
            let _ = meta.value()?.parse::<syn::Expr>()?;
        } else if meta.input.peek(syn::token::Paren) {
            meta.parse_nested_meta(|inner| {
                if inner.input.peek(Token![=]) {
                    let _ = inner.value()?.parse::<syn::Expr>()?;
                }
                Ok(())
            })?;
        }

        Ok(())
    })
}

fn append_field_builders(
    field: &FieldDoc,
    doc_builders: &mut Vec<TokenStream>,
    value_type_builders: &mut Vec<TokenStream>,
    default_display_builders: &mut Vec<TokenStream>,
    sensitive_builders: &mut Vec<TokenStream>,
    pending_builders: &mut Vec<TokenStream>,
    warning_builders: &mut Vec<TokenStream>,
    migration_builders: &mut Vec<TokenStream>,
    deprecation_builders: &mut Vec<TokenStream>,
    danger_builders: &mut Vec<TokenStream>,
) {
    if field.serde.skip {
        return;
    }

    let display_name_lit = LitStr::new(&field.display_name, field.span);

    if !field.serde.flatten {
        let key = field
            .auto_doc
            .key
            .as_ref()
            .expect("non-flatten fields must have an AutoDoc key");

        doc_builders.push(push_translated_entry("all_doc", &display_name_lit, key));
        value_type_builders.push(push_value_type_entry(
            "all_value_types",
            &display_name_lit,
            field_value_type_name(&field.field_ty, field.auto_doc.has_sub),
        ));
        if let Some(default_display) = &field.auto_doc.default_display {
            default_display_builders.push(push_literal_entry(
                "all_default_display",
                &display_name_lit,
                default_display,
            ));
        }
        if field.auto_doc.sensitive {
            sensitive_builders.push(quote! {
                all_sensitive.push(::std::string::String::from(#display_name_lit));
            });
        }
        push_optional_translated_entry(
            pending_builders,
            "all_pending",
            &display_name_lit,
            field.auto_doc.pending_deprecated.as_deref(),
        );
        push_optional_translated_entry(
            warning_builders,
            "all_warning",
            &display_name_lit,
            field.auto_doc.warning.as_deref(),
        );
        push_optional_translated_entry(
            migration_builders,
            "all_migration",
            &display_name_lit,
            field.auto_doc.migration_notice.as_deref(),
        );
        push_optional_translated_entry(
            deprecation_builders,
            "all_deprecation",
            &display_name_lit,
            field.auto_doc.deprecation.as_deref(),
        );
        push_optional_translated_entry(
            danger_builders,
            "all_danger",
            &display_name_lit,
            field.auto_doc.danger.as_deref(),
        );
    }

    if !field.auto_doc.has_sub && !field.serde.flatten {
        return;
    }

    let prefix = if field.serde.flatten {
        None
    } else {
        Some(display_name_lit)
    };

    doc_builders.push(push_recursive_entries(
        "all_doc",
        "doc_fields",
        &field.field_ty,
        prefix.as_ref(),
    ));
    value_type_builders.push(push_recursive_type_entries(
        "all_value_types",
        &field.field_ty,
        prefix.as_ref(),
    ));
    default_display_builders.push(push_recursive_entries(
        "all_default_display",
        "default_display_fields",
        &field.field_ty,
        prefix.as_ref(),
    ));
    sensitive_builders.push(push_recursive_sensitive_entries(
        "all_sensitive",
        &field.field_ty,
        prefix.as_ref(),
    ));
    pending_builders.push(push_recursive_entries(
        "all_pending",
        "pending_deprecated_fields",
        &field.field_ty,
        prefix.as_ref(),
    ));
    warning_builders.push(push_recursive_entries(
        "all_warning",
        "warning_fields",
        &field.field_ty,
        prefix.as_ref(),
    ));
    migration_builders.push(push_recursive_entries(
        "all_migration",
        "migration_notice_fields",
        &field.field_ty,
        prefix.as_ref(),
    ));
    deprecation_builders.push(push_recursive_entries(
        "all_deprecation",
        "deprecation_fields",
        &field.field_ty,
        prefix.as_ref(),
    ));
    danger_builders.push(push_recursive_entries(
        "all_danger",
        "danger_fields",
        &field.field_ty,
        prefix.as_ref(),
    ));
}

fn push_translated_entry(target: &str, display_name: &LitStr, i18n_key: &str) -> TokenStream {
    let target = Ident::new(target, Span::call_site());
    let i18n_key = LitStr::new(i18n_key, Span::call_site());

    quote! {
        ::qexed_config_autodoc::autodoc_push_translated(
            &mut #target,
            #display_name,
            #i18n_key,
            lang,
        );
    }
}

fn push_literal_entry(target: &str, display_name: &LitStr, value: &str) -> TokenStream {
    let target = Ident::new(target, Span::call_site());
    let value = LitStr::new(value, Span::call_site());

    quote! {
        ::qexed_config_autodoc::autodoc_push_literal(&mut #target, #display_name, #value);
    }
}

fn push_value_type_entry(target: &str, display_name: &LitStr, value_type: &str) -> TokenStream {
    let target = Ident::new(target, Span::call_site());
    let value_type = LitStr::new(value_type, Span::call_site());

    quote! {
        ::qexed_config_autodoc::autodoc_push_value_type(&mut #target, #display_name, #value_type);
    }
}

fn push_optional_translated_entry(
    builders: &mut Vec<TokenStream>,
    target: &str,
    display_name: &LitStr,
    i18n_key: Option<&str>,
) {
    if let Some(i18n_key) = i18n_key {
        builders.push(push_translated_entry(target, display_name, i18n_key));
    }
}

fn push_recursive_type_entries(
    target: &str,
    field_ty: &Type,
    prefix: Option<&LitStr>,
) -> TokenStream {
    let target = Ident::new(target, Span::call_site());
    let trait_target = autodoc_trait_target_type(field_ty);

    match prefix {
        Some(prefix) => quote! {
            ::qexed_config_autodoc::autodoc_extend_prefixed_value_types(
                &mut #target,
                #prefix,
                <#trait_target as ::qexed_config_autodoc::AutoDocConfigTrait>::field_value_types(),
            );
        },
        None => quote! {
            #target.extend(<#trait_target as ::qexed_config_autodoc::AutoDocConfigTrait>::field_value_types());
        },
    }
}

fn push_recursive_entries(
    target: &str,
    method: &str,
    field_ty: &Type,
    prefix: Option<&LitStr>,
) -> TokenStream {
    let target = Ident::new(target, Span::call_site());
    let method = Ident::new(method, Span::call_site());
    let trait_target = autodoc_trait_target_type(field_ty);

    match prefix {
        Some(prefix) => quote! {
            ::qexed_config_autodoc::autodoc_extend_prefixed_strings(
                &mut #target,
                #prefix,
                <#trait_target as ::qexed_config_autodoc::AutoDocConfigTrait>::#method(lang),
            );
        },
        None => quote! {
            #target.extend(<#trait_target as ::qexed_config_autodoc::AutoDocConfigTrait>::#method(lang));
        },
    }
}

fn push_recursive_sensitive_entries(
    target: &str,
    field_ty: &Type,
    prefix: Option<&LitStr>,
) -> TokenStream {
    let target = Ident::new(target, Span::call_site());
    let trait_target = autodoc_trait_target_type(field_ty);

    match prefix {
        Some(prefix) => quote! {
            ::qexed_config_autodoc::autodoc_extend_prefixed_sensitive(
                &mut #target,
                #prefix,
                <#trait_target as ::qexed_config_autodoc::AutoDocConfigTrait>::sensitive_fields(),
            );
        },
        None => quote! {
            #target.extend(<#trait_target as ::qexed_config_autodoc::AutoDocConfigTrait>::sensitive_fields());
        },
    }
}

fn field_value_type_name(field_ty: &Type, has_sub: bool) -> &'static str {
    if type_is_generic(field_ty, "Vec")
        || type_is_generic(field_ty, "HashSet")
        || type_is_generic(field_ty, "BTreeSet")
    {
        return "array";
    }
    if let Some(inner_ty) = generic_inner_type(field_ty, "Option") {
        return field_value_type_name(inner_ty, has_sub);
    }
    if type_is_generic(field_ty, "HashMap") || type_is_generic(field_ty, "BTreeMap") {
        return "object";
    }
    if has_sub {
        return "object";
    }

    let Some(ident) = last_type_ident(field_ty) else {
        return "unknown";
    };
    match ident.as_str() {
        "bool" => "boolean",
        "usize" | "u64" | "u32" | "u16" | "u8" | "isize" | "i64" | "i32" | "i16" | "i8" => {
            "integer"
        }
        "f64" | "f32" => "number",
        "String" | "str" | "PathBuf" | "Uuid" | "Duration" => "string",
        name if enum_like_type_name(name) => "string",
        _ => "object",
    }
}

fn enum_like_type_name(name: &str) -> bool {
    [
        "Algorithm",
        "Color",
        "Engine",
        "Generator",
        "Kind",
        "Level",
        "Mode",
        "Overlay",
        "Provider",
        "Selector",
        "Source",
        "Storage",
    ]
    .iter()
    .any(|suffix| name.ends_with(suffix))
}

fn type_is_generic(field_ty: &Type, generic_name: &str) -> bool {
    generic_inner_type(field_ty, generic_name).is_some()
}

fn generic_inner_type<'a>(field_ty: &'a Type, generic_name: &str) -> Option<&'a Type> {
    let Type::Path(type_path) = field_ty else {
        return None;
    };
    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident != generic_name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
        return None;
    };
    Some(inner_ty)
}

fn last_type_ident(field_ty: &Type) -> Option<String> {
    let Type::Path(type_path) = field_ty else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn autodoc_trait_target_type(field_ty: &Type) -> TokenStream {
    let Type::Path(type_path) = field_ty else {
        return quote! { #field_ty };
    };
    let Some(last_segment) = type_path.path.segments.last() else {
        return quote! { #field_ty };
    };
    if last_segment.ident != "Vec" {
        return quote! { #field_ty };
    }
    let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
        return quote! { #field_ty };
    };
    let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
        return quote! { #field_ty };
    };
    quote! { #inner_ty }
}

fn push_error(errors: &mut Option<syn::Error>, error: syn::Error) {
    if let Some(errors) = errors {
        errors.combine(error);
    } else {
        *errors = Some(error);
    }
}
