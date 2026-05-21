use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, FieldsNamed, FieldsUnnamed, Lit, Meta, parse_macro_input};
#[proc_macro_derive(I18nErrorDisplay, attributes(error))]
pub fn i18n_error_display_derive(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let enum_name = &input.ident;

    // 检查是否是枚举
    match &input.data {
        Data::Enum(data) => return enum_derive(enum_name, &data.variants),
        _ => {
            quote! {
                 panic!("AutoEnum只能用于枚举类型"),
            }.into()
        }
    }
}
fn enum_derive(
    name: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
) -> TokenStream {
    let match_arms: Vec<TokenStream2> = variants
        .iter()
        .map(|variant| {
            let variant_ident = &variant.ident;
            let fields = &variant.fields;
            let pattern = match &variant.fields {
                // 单元变体：没有字段
                Fields::Unit => {
                    quote! { #name::#variant_ident }
                }
                // 元组变体：有未命名字段
                Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
                    // 为每个字段生成通配符模式
                    let field_patterns: Vec<_> = unnamed
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            // 使用下划线忽略字段值
                            let field_name = syn::Ident::new(&format!("field_{}", i), variant.span());
                            quote! { #field_name }
                        })
                        .collect();
                    
                    quote! { #name::#variant_ident(#(#field_patterns),*) }
                }
                // 命名结构体变体：有命名字段
                Fields::Named(FieldsNamed { named, .. }) => {
                    // 为每个字段生成通配符模式
                    let field_patterns: Vec<_> = named
                        .iter()
                        .map(|field| {
                            let field_name = &field.ident.as_ref().unwrap();
                            quote! { #field_name: _ }
                        })
                        .collect();
                    
                    quote! { #name::#variant_ident { #(#field_patterns),* } }
                }
            };
            match variant
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("error"))
            {
                Some(attr) => {
                    // match &variant.fields {
                    //     Fields::Named(fields_named) => todo!(),
                    //     Fields::Unnamed(fields_unnamed) => todo!(),
                    //     Fields::Unit => todo!(),
                    // }
                    // #[error("qexed_tcp_connect.addr_in_use",addr = field_0.to_string(),ip = field_0.ip().to_string(),port = field_0.port())]
                    let meta = attr.meta.clone();
                    match &meta {
                        Meta::Path(_)=>{
                            quote! {
                                #pattern => ::core::fmt::Display::fmt(&::rust_i18n::t!("#pattern"), f),
                            }
                        }
                        Meta::NameValue(meta_name_value) => {
                            quote! {
                                #pattern => ::core::fmt::Display::fmt(&::rust_i18n::t!("#meta_name_value"), f),
                            }
                        },
                        Meta::List(meta_list) => {
                            let parser = Punctuated::<syn::Expr, Comma>::parse_separated_nonempty;
                            match parser.parse2(meta_list.tokens.clone()) {
                                Ok(exprs) => {
                                    if exprs.is_empty() {
                                        return quote! {
                                            #pattern => ::core::fmt::Display::fmt(&::rust_i18n::t!("#pattern"), f),
                                        };
                                    }
                                    let error_code = match &exprs[0] {
                                        syn::Expr::Lit(expr_lit) => {
                                            match &expr_lit.lit {
                                                syn::Lit::Str(lit_str) => lit_str.clone(),
                                                _ => {
                                                    return quote! {
                                                        #pattern => panic("First argument must be a string literal (error code)"),
                                                    };
                                                }
                                            }
                                        }
                                        _ =>{
                                            return quote! {
                                                #pattern => panic("First argument must be a string literal (error code)"),
                                            };
                                        }
                                    };
                                    let args = exprs.iter().skip(1);
                                    quote! {
                                        #pattern => ::core::fmt::Display::fmt(&::rust_i18n::t!(#error_code,#(#args),*), f),
                                    }
                                },
                                Err(e) =>{
                                    quote! {
                                        #pattern => panic("Failed to parse [error] attribute"),
                                    }
                                },
                            }
                        },
                        
                    }

                }
                None => {
                    quote! {
                        #pattern => ::core::fmt::Display::fmt(&self, f),
                    }
                }
            }
        })
        .collect();
    quote! {
        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    #(#match_arms)*
                }
            }
        }
    }
    .into()
}
