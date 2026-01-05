use qexed_config::app::qexed_tcp_connect_app::ForwardingMode;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

impl super::LogicTask {
    pub fn bungeecord_decode_json(&self, data: &str) -> Result<Vec<qexed_protocol::to_client::login::success::Properties>> {
        let properties: Vec<qexed_protocol::to_client::login::success::Properties> = serde_json::from_str(data)
            .context("Failed to parse BungeeCord properties JSON")?;
        Ok(properties)
    }
    
    /// 解码纹理数据（针对name为"textures"的属性）
    pub fn decode_textures_data(&self, property: &qexed_protocol::to_client::login::success::Properties) -> Result<Option<TexturesData>> {
        if property.name != "textures" {
            return Ok(None);
        }
        
        // Base64解码value字段
        let decoded_bytes = STANDARD.decode(&property.value)
            .context("Failed to Base64 decode textures data")?;
        
        // 将解码后的字节转换为字符串
        let decoded_str = String::from_utf8(decoded_bytes)
            .context("Failed to convert decoded bytes to UTF-8 string")?;
        
        // 解析为TexturesData结构体
        let textures_data: TexturesData = serde_json::from_str(&decoded_str)
            .context("Failed to parse textures JSON")?;
        
        Ok(Some(textures_data))
    }
    
    /// 完整的BungeeCord数据处理流程
    pub fn process_bungeecord_data(&mut self, data: &str) -> Result<ProcessedBungeeCordData> {
        let properties = self.bungeecord_decode_json(data)?;
        self.player_properties = properties.clone();
        let mut textures_data = None;
        let mut bungeeguard_token = None;
        
        for property in &properties {
            match property.name.as_str() {
                "textures" => {
                    textures_data = self.decode_textures_data(property)?;
                }
                "bungeeguard-token" => {
                    bungeeguard_token = Some(property.value.clone());
                }
                _ => {
                    // 可以处理其他自定义属性
                    log::debug!("Unknown BungeeCord property: {}", property.name);
                }
            }
        }
        
        Ok(ProcessedBungeeCordData {
            properties,
            textures_data,
            bungeeguard_token,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TexturesData {
    pub timestamp: u64,
    #[serde(rename = "profileId")]
    pub profile_id: String,
    #[serde(rename = "profileName")]
    pub profile_name: String,
    #[serde(rename = "signatureRequired")]
    pub signature_required: bool,
    pub textures: Textures,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Textures {
    #[serde(rename = "SKIN")]
    pub skin: Option<Texture>,
    #[serde(rename = "CAPE")]
    pub cape: Option<Texture>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Texture {
    pub url: String,
}

/// 处理后的BungeeCord数据结果
#[derive(Debug)]
pub struct ProcessedBungeeCordData {
    pub properties: Vec<qexed_protocol::to_client::login::success::Properties>,
    pub textures_data: Option<TexturesData>,
    pub bungeeguard_token: Option<String>,
}

// // 为qexed_protocol::to_client::login::success::Properties添加一些实用方法
// impl qexed_protocol::to_client::login::success::Properties {
//     /// 检查是否是纹理属性
//     pub fn is_textures(&self) -> bool {
//         self.name == "textures"
//     }
    
//     /// 检查是否是BungeeGuard令牌属性
//     pub fn is_bungeeguard_token(&self) -> bool {
//         self.name == "bungeeguard-token"
//     }
// }

// 使用示例
impl super::LogicTask {
    pub async fn handle_bungeecord_properties(&mut self, properties_json: &str) -> Result<()> {
        let processed_data = self.process_bungeecord_data(properties_json)?;
        
        // 处理纹理数据
        if let Some(textures) = &processed_data.textures_data {
            self.player_name = textures.profile_name.clone();
            log::info!("Player textures - Name: {}, ID: {}", 
                      textures.profile_name, textures.profile_id);
            
            if let Some(skin) = &textures.textures.skin {
                log::debug!("Skin URL: {}", skin.url);
            }
            
            if let Some(cape) = &textures.textures.cape {
                log::debug!("Cape URL: {}", cape.url);
            }
            self.proxy_is_login = true;
        } else if self.online_mode{
            return Err(anyhow::anyhow!("Minecraft 正版验证失败"));
        }
        
        // 处理BungeeGuard令牌
        if let Some(token) = &processed_data.bungeeguard_token {
            if self.proxy_protocol == ForwardingMode::BungeeCord{
                return Err(anyhow::anyhow!("代理模式配置错误"));
            }
            if self.proxy_token==""{
                return Err(anyhow::anyhow!("BungeeGuard Token 未配置"));
            }
            if *token != self.proxy_token{
                return Err(anyhow::anyhow!("BungeeGuard Token 验证失败"));
            }
            
            // 这里可以添加令牌验证逻辑
        } else {
            if self.proxy && self.proxy_protocol == ForwardingMode::BungeeGuard{
                // 这里我们塞一下反作弊模块来封禁IP
                return Err(anyhow::anyhow!("BungeeGuard 验证失败"));
            }

        }
        
        // // 处理其他属性
        // for property in &processed_data.properties {
        //     if !property.is_textures() && !property.is_bungeeguard_token() {
        //         log::info!("Additional property - {}: {}", property.name, property.value);
        //     }
        // }
        
        Ok(())
    }
}