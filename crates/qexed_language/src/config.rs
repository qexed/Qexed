use std::collections::HashMap;

use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};


/// ```autodoc
/// <Name>qexed.crates.language.config.LanguageConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "language")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct LanguageConfig {
    /// ```autodoc
    /// <Name>qexed.crates.language.config.LanguageConfig.enable</Name>
    /// <Default>true</Default>
    /// ```
    pub enable: bool,
    /// ```autodoc
    /// <Name>qexed.crates.language.config.LanguageConfig.language</Name>
    /// <Default>zh-CN</Default>
    /// ```
    pub language: String,
    /// 线上下载语言文件的服务器
    /// 默认是官方文档站，不配置则在缺失语言文件时，使用翻译键
    /// 可使用 qexed_language_server 自行开发部署,支持网页上自行创建语言文件并自行处理翻译
    /// 语言文件服务器可以是纯静态的
    /// 此外插件的语言配置有些特殊，若默认服务器没有插件的翻译文件，则从插件默认配置的语言服务器获取
    /// 路由规则:
    /// `GET /api/v1/qexed/{commit}/language.json` 获取commit对应版本的语言支持列表
    /// `GET /api/v1/qexed/{commit}/{language}.json` 获取commit对应版本的对应语言翻译文本
    /// `GET /api/v1/{author}/{plugin}/{commit}/language.json` 获取对应作者的指定插件的commit对应版本的语言支持列表
    /// `GET /api/v1/{author}/{plugin}/{commit}/{language}.json` 获取对应作者的指定插件的commit对应版本的对应语言翻译文本
    /// ```autodoc
    /// <Name>qexed.crates.language.config.LanguageConfig.server_url</Name>
    /// <Default>https://language.qexed.com/</Default>
    /// ```
    pub server_url: Option<String>,
    /// 当server_url和插件自带的无法获取了，我们根据自定义插件作者的服务器获取
    /// ```autodoc
    /// <Name>qexed.crates.language.config.LanguageConfig.plugin_author_server_url</Name>
    /// ```
    pub plugin_author_server_url: HashMap<String,String>,
    /// 如果连作者的都无法获取了，只能给插件单独配置单独的服务器了
    /// ```autodoc
    /// <Name>qexed.crates.language.config.LanguageConfig.plugin_server_url</Name>
    /// ```
    pub plugin_server_url: HashMap<String,HashMap<String,String>>,
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            enable:true,
            language:"zh-CN".to_string(),
            server_url:Some("https://language.qexed.com/".to_string()),
            plugin_author_server_url:Default::default(),
            plugin_server_url:Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_server_url_map_schema() {
        let schema = LanguageConfig::schema();
        // 平铺 map:plugin_author_server_url
        let flat = schema.fields.iter().find(|f| f.path == "plugin_author_server_url").unwrap();
        assert_eq!(flat.value_type, "map<string,String>");
        // 嵌套 map:plugin_server_url(作者服务器失效后按插件兜底的二级表)
        let nested = schema.fields.iter().find(|f| f.path == "plugin_server_url").unwrap();
        assert_eq!(nested.value_type, "map<string,map<string,String>>");
        // 校验:平铺通过/拒绝
        let ok = qexed_doc::serde_json::json!({"myplug": "https://plug.example.com/"});
        assert!(<HashMap<String, String> as qexed_doc::DocValue>::validate(&ok).is_ok());
        let bad = qexed_doc::serde_json::json!({"myplug": 42});
        assert!(<HashMap<String, String> as qexed_doc::DocValue>::validate(&bad).is_err());
        // 嵌套:值不是 object → 拒绝;完整二级结构 → 通过
        let shallow = qexed_doc::serde_json::json!({"myplug": "https://not-a-map.example.com/"});
        assert!(<HashMap<String, HashMap<String, String>> as qexed_doc::DocValue>::validate(&shallow).is_err());
        let deep = qexed_doc::serde_json::json!({"myplug": {"zh-CN": "https://zh.plug.example.com/"}});
        assert!(<HashMap<String, HashMap<String, String>> as qexed_doc::DocValue>::validate(&deep).is_ok());
    }

    #[test]
    fn storage_dir_under_config_path() {
        let tmp = std::env::temp_dir().join("qexed_lang_test_cfg");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        qexed_config::init_config_path(tmp.clone()).unwrap();
        let dir = crate::storage_dir().unwrap();
        assert_eq!(dir, tmp.join("languages"));
        // 未初始化时不应 panic(新进程内 init_config_path 已 set,这里只验证派生路径正确)
    }
}
