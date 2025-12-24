use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Meta, Expr, Lit};
use syn::{ Data, Fields};
#[proc_macro_attribute]
pub fn packet(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 解析输入的结构体
    let input = parse_macro_input!(item as DeriveInput);
    
    // 解析属性参数
    let attr_meta: Meta = syn::parse(attr).expect("Failed to parse attribute");
    
    // 提取 packet id
    let packet_id = match &attr_meta {
        Meta::NameValue(meta_name_value) => {
            if meta_name_value.path.is_ident("id") {
                if let Expr::Lit(expr_lit) = &meta_name_value.value {
                    if let Lit::Int(lit_int) = &expr_lit.lit {
                        lit_int.base10_parse::<u32>().expect("Invalid packet id")
                    } else {
                        panic!("Packet id must be an integer literal");
                    }
                } else {
                    panic!("Packet id must be a literal");
                }
            } else {
                panic!("Expected 'id' parameter");
            }
        }
        Meta::List(_meta_list) => {
            // 处理列表形式，如 #[packet(id = 0x00)]
            // 这里简化处理，只查找 id 参数
            panic!("Please use #[packet(id = ...)] format");
        }
        _ => {
            panic!("Invalid attribute format");
        }
    };
    
    // 获取结构体名称
    let struct_name = &input.ident;
    
    // 获取结构体的字段
    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(fields_named),
        ..
    }) = &input.data {
        &fields_named.named
    } else {
        panic!("packet macro only supports structs with named fields");
    };
    
    // 收集字段名
    let field_names: Vec<_> = fields.iter()
        .filter_map(|f| f.ident.as_ref())
        .collect();
    
    // 生成实现代码
    let expanded = quote! {
        #input
        
        impl qexed_packet::Packet for #struct_name {
            const ID: u32 = #packet_id;
            
            fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
                #(self.#field_names.serialize(w)?;)*
                Ok(())
            }
            
            fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
                #(self.#field_names.deserialize(r)?;)*
                Ok(())
            }
        }
    };
    
    TokenStream::from(expanded)
}



#[proc_macro_attribute]
pub fn substruct(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 解析输入的结构体
    let input = parse_macro_input!(item as DeriveInput);
    
    // 获取结构体名称
    let struct_name = &input.ident;
    
    // 获取结构体的泛型参数
    let generics = &input.generics;
    
    // 为结构体生成 PacketCodec 实现
    let expanded = match &input.data {
        Data::Struct(data_struct) => {
            match &data_struct.fields {
                Fields::Named(fields_named) => {
                    // 处理有命名字段的结构体
                    let field_names: Vec<_> = fields_named.named.iter()
                        .map(|f| f.ident.as_ref().unwrap())
                        .collect();
                    
                    // 生成序列化代码
                    let serialize_fields = quote! {
                        #(self.#field_names.serialize(w)?;)*
                    };
                    
                    // 生成反序列化代码
                    let deserialize_fields = quote! {
                        #(self.#field_names.deserialize(r)?;)*
                    };
                    
                    // 生成完整的实现
                    quote! {
                        #input
                        
                        impl #generics qexed_packet::PacketCodec for #struct_name #generics {
                            fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
                                #serialize_fields
                                Ok(())
                            }
                            
                            fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
                                #deserialize_fields
                                Ok(())
                            }
                        }
                    }
                }
                Fields::Unnamed(fields_unnamed) => {
                    // 处理元组结构体
                    let field_indices: Vec<_> = (0..fields_unnamed.unnamed.len())
                        .map(syn::Index::from)
                        .collect();
                    
                    // 生成序列化代码
                    let serialize_fields = quote! {
                        #(self.#field_indices.serialize(w)?;)*
                    };
                    
                    // 生成反序列化代码
                    let deserialize_fields = quote! {
                        #(self.#field_indices.deserialize(r)?;)*
                    };
                    
                    // 生成完整的实现
                    quote! {
                        #input
                        
                        impl #generics qexed_packet::PacketCodec for #struct_name #generics {
                            fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
                                #serialize_fields
                                Ok(())
                            }
                            
                            fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
                                #deserialize_fields
                                Ok(())
                            }
                        }
                    }
                }
                Fields::Unit => {
                    // 处理单元结构体
                    quote! {
                        #input
                        
                        impl #generics qexed_packet::PacketCodec for #struct_name #generics {
                            fn serialize(&self, _w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
                                Ok(())
                            }
                            
                            fn deserialize(&mut self, _r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
                                Ok(())
                            }
                        }
                    }
                }
            }
        }
        Data::Enum(_) => {
            panic!("substruct macro does not support enums");
        }
        Data::Union(_) => {
            panic!("substruct macro does not support unions");
        }
    };
    
    TokenStream::from(expanded)
}


#[proc_macro_attribute]
pub fn subenum(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 解析输入的枚举
    let input = parse_macro_input!(item as DeriveInput);
    
    // 获取枚举名称
    let enum_name = &input.ident;
    
    // 获取枚举的泛型参数
    let generics = &input.generics;
    
    // 为枚举生成PacketCodec实现
    let expanded = match &input.data {
        Data::Enum(data_enum) => {
            // 获取所有变体
            let variants: Vec<_> = data_enum.variants.iter().collect();
            
            // 检查是否有Unknown变体
            let has_unknown = variants.iter()
                .any(|v| v.ident == "Unknown");
            
            // 为每个变体生成匹配代码
            let mut serialize_arms = Vec::new();
            let mut deserialize_arms = Vec::new();
            
            for (index, variant) in variants.iter().enumerate() {
                let variant_name = &variant.ident;
                let id = index as i32;
                
                // 处理变体的字段
                match &variant.fields {
                    Fields::Unit => {
                        // 单元变体（如 Unknown）
                        if variant_name == "Unknown" {
                            serialize_arms.push(quote! {
                                #enum_name::#variant_name => {
                                    return Err(anyhow::anyhow!("Cannot serialize unknown variant"));
                                }
                            });
                        } else {
                            serialize_arms.push(quote! {
                                #enum_name::#variant_name => {
                                    let id = qexed_packet::net_types::VarInt(#id);
                                    id.serialize(w)?;
                                }
                            });
                        }
                        
                        // 反序列化分支
                        if variant_name == "Unknown" && has_unknown {
                            // Unknown 通常是最后的分支
                            continue;
                        } else {
                            deserialize_arms.push(quote! {
                                #id => {
                                    *self = #enum_name::#variant_name;
                                }
                            });
                        }
                    }
                    Fields::Unnamed(fields_unnamed) => {
                        // 元组变体
                        if fields_unnamed.unnamed.len() == 1 {
                            // 单个字段的元组变体
                            let field_type = &fields_unnamed.unnamed[0].ty;
                            
                            serialize_arms.push(quote! {
                                #enum_name::#variant_name(data) => {
                                    let id = qexed_packet::net_types::VarInt(#id);
                                    id.serialize(w)?;
                                    data.serialize(w)?;
                                }
                            });
                            
                            deserialize_arms.push(quote! {
                                #id => {
                                    let mut data: #field_type = Default::default();
                                    data.deserialize(r)?;
                                    *self = #enum_name::#variant_name(data);
                                }
                            });
                        } else {
                            // 多个字段的元组变体
                            let field_count = fields_unnamed.unnamed.len();
                            let field_types: Vec<_> = fields_unnamed.unnamed.iter()
                                .map(|f| &f.ty)
                                .collect();
                            let field_names: Vec<_> = (0..field_count)
                                .map(|i| format_ident!("field{}", i))
                                .collect();
                            
                            // 序列化分支
                            serialize_arms.push(quote! {
                                #enum_name::#variant_name(#(#field_names),*) => {
                                    let id = qexed_packet::net_types::VarInt(#id);
                                    id.serialize(w)?;
                                    #(#field_names.serialize(w)?;)*
                                }
                            });
                            
                            // 反序列化分支
                            deserialize_arms.push(quote! {
                                #id => {
                                    #(let mut #field_names: #field_types = Default::default();)*
                                    #(#field_names.deserialize(r)?;)*
                                    *self = #enum_name::#variant_name(#(#field_names),*);
                                }
                            });
                        }
                    }
                    Fields::Named(fields_named) => {
                        // 具名结构体变体
                        let field_names: Vec<_> = fields_named.named.iter()
                            .map(|f| f.ident.as_ref().unwrap())
                            .collect();
                        let field_types: Vec<_> = fields_named.named.iter()
                            .map(|f| &f.ty)
                            .collect();
                        
                        serialize_arms.push(quote! {
                            #enum_name::#variant_name{ #(#field_names),* } => {
                                let id = qexed_packet::net_types::VarInt(#id);
                                id.serialize(w)?;
                                #(#field_names.serialize(w)?;)*
                            }
                        });
                        
                        deserialize_arms.push(quote! {
                            #id => {
                                #(let mut #field_names: #field_types = Default::default();)*
                                #(#field_names.deserialize(r)?;)*
                                *self = #enum_name::#variant_name {
                                    #(#field_names),*
                                };
                            }
                        });
                    }
                }
            }
            
            // 添加Unknown分支
            if has_unknown {
                deserialize_arms.push(quote! {
                    _ => {
                        *self = #enum_name::Unknown;
                    }
                });
            } else {
                deserialize_arms.push(quote! {
                    _ => {
                        return Err(anyhow::anyhow!("Unknown variant id: {}", id.0));
                    }
                });
            }
            
            // 生成 Default 实现
            let default_impl = if has_unknown {
                quote! {
                    impl #generics Default for #enum_name #generics {
                        fn default() -> Self {
                            #enum_name::Unknown
                        }
                    }
                }
            } else {
                quote! {
                    impl #generics Default for #enum_name #generics {
                        fn default() -> Self {
                            // 如果没有Unknown，使用第一个变体
                            match 0 {
                                #( #deserialize_arms )*
                            }
                        }
                    }
                }
            };
            
            // 生成完整的实现
            quote! {
                #input
                
                #default_impl
                
                impl #generics qexed_packet::PacketCodec for #enum_name #generics {
                    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
                        match self {
                            #(#serialize_arms)*
                        }
                        Ok(())
                    }
                    
                    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
                        // 读取变体ID
                        let mut id: qexed_packet::net_types::VarInt = qexed_packet::net_types::VarInt(-1);
                        id.deserialize(r)?;
                        
                        // 根据ID反序列化对应的变体
                        match id.0 {
                            #(#deserialize_arms)*
                        }
                        
                        Ok(())
                    }
                }
            }
        }
        Data::Struct(_) => {
            panic!("subenum macro only supports enums, use substruct for structs");
        }
        Data::Union(_) => {
            panic!("subenum macro does not support unions");
        }
    };
    
    TokenStream::from(expanded)
}