// qexed_config_macros/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};
use syn::{
    Expr, Token,
    parse::{Parse, ParseStream},
};
use syn::{ItemStruct, LitStr};
mod autodoc;

#[proc_macro_derive(AutoDoc, attributes(AutoDoc, serde))]
pub fn derive_autodoc(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    autodoc::expand(input).into()
}
/// 自动为枚举生成常用Trait实现的简化宏
#[proc_macro_derive(AutoEnum, attributes(default, display))]
pub fn auto_enum_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = &input.ident;

    // 检查是否是枚举
    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("AutoEnum只能用于枚举类型"),
    };

    // 收集变体信息
    let mut variant_idents = Vec::new();
    let mut default_variant = None;

    for variant in variants {
        match &variant.fields {
            Fields::Unit => {
                let ident = &variant.ident;
                variant_idents.push(ident);

                // 检查是否有#[default]属性
                for attr in &variant.attrs {
                    if attr.path().is_ident("default") {
                        default_variant = Some(ident);
                    }
                }
            }
            _ => panic!("AutoEnum只支持无字段枚举变体"),
        }
    }

    // 生成代码
    let mut output = proc_macro2::TokenStream::new();

    // 生成Default实现
    if let Some(default_ident) = default_variant {
        output.extend(quote! {
            impl std::default::Default for #enum_name {
                fn default() -> Self {
                    #enum_name::#default_ident
                }
            }
        });
    }

    // 生成Display实现
    let display_arms: Vec<_> = variant_idents
        .iter()
        .map(|ident| {
            let display_text = ident.to_string();
            quote! {
                #enum_name::#ident => write!(f, #display_text),
            }
        })
        .collect();

    output.extend(quote! {
        impl std::fmt::Display for #enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #(#display_arms)*
                }
            }
        }
    });

    // 生成FromStr实现
    let from_str_arms: Vec<_> = variant_idents
        .iter()
        .map(|ident| {
            let ident_str = ident.to_string().to_lowercase();
            quote! {
                #ident_str => Ok(#enum_name::#ident),
            }
        })
        .collect();

    output.extend(quote! {
        impl std::str::FromStr for #enum_name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.to_lowercase().as_str() {
                    #(#from_str_arms)*
                    _ => Err(format!("无效的{}值: '{}'", stringify!(#enum_name), s)),
                }
            }
        }
    });

    TokenStream::from(output)
}
/// 解析 `#[AppConfig(...)]` 的参数
/// 解析 `#[AppConfig(...)]` 的参数
struct AppConfigArgs {
    path: String,
    name: String,
}

impl Parse for AppConfigArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut path = None;
        let mut name = None;

        // 方法：先尝试解析一个字符串字面量（位置参数模式）
        // 如果失败，则说明是键值对模式
        let lookahead = input.lookahead1();
        if lookahead.peek(LitStr) {
            // 位置参数: "path", "name"
            let path_lit: LitStr = input.parse()?;
            path = Some(path_lit.value());
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
            let name_lit: LitStr = input.parse()?;
            name = Some(name_lit.value());
        } else {
            // 键值对模式: path = "...", name = "..."
            while !input.is_empty() {
                let ident: syn::Ident = input.parse()?;
                input.parse::<Token![=]>()?;
                let expr: Expr = input.parse()?;
                let lit = match expr {
                    Expr::Lit(lit) => lit,
                    _ => return Err(input.error("expected string literal")),
                };
                let value = match lit.lit {
                    syn::Lit::Str(s) => s.value(),
                    _ => return Err(input.error("expected string literal")),
                };
                if ident == "path" {
                    path = Some(value);
                } else if ident == "name" {
                    name = Some(value);
                } else {
                    return Err(input.error(format!("unknown key `{}`", ident)));
                }
                // 允许逗号分隔
                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                } else {
                    break;
                }
            }
        }

        Ok(AppConfigArgs {
            path: path.ok_or_else(|| input.error("missing path"))?,
            name: name.ok_or_else(|| input.error("missing name"))?,
        })
    }
}
#[proc_macro_attribute]
pub fn app_config(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AppConfigArgs);
    let input = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;
    let path = args.path;
    let name = args.name;

    let expanded = quote! {
        #input

        impl ::qexed_config::tool::AppConfigTrait for #struct_name {
            const PATH: &'static str = #path;
            const NAME: &'static str = #name;
        }
    };
    expanded.into()
}
