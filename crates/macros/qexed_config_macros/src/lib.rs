// qexed_config_macros/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields};

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
            },
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
    let display_arms: Vec<_> = variant_idents.iter().map(|ident| {
        let display_text = ident.to_string();
        quote! {
            #enum_name::#ident => write!(f, #display_text),
        }
    }).collect();
    
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
    let from_str_arms: Vec<_> = variant_idents.iter().map(|ident| {
        let ident_str = ident.to_string().to_lowercase();
        quote! {
            #ident_str => Ok(#enum_name::#ident),
        }
    }).collect();
    
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