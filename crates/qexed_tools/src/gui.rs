use anyhow::{Context, Result};
use http::{Response, StatusCode, header};
use std::collections::BTreeMap;
use tauri::{Url, WebviewUrl, WebviewWindowBuilder};

const GUI_PROTOCOL: &str = "qexed-tools";
const GUI_CSP: &str = "default-src 'self'; connect-src ipc: http://ipc.localhost https://ipc.localhost; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'";

#[derive(Debug, Clone)]
pub struct GuiApp {
    pub title: String,
    pub html: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiLanguage {
    ZhCn,
    En,
}

impl GuiLanguage {
    pub fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
        if normalized.starts_with("zh") {
            Some(Self::ZhCn)
        } else if normalized.starts_with("en") {
            Some(Self::En)
        } else {
            None
        }
    }
}

impl AsRef<str> for GuiLanguage {
    fn as_ref(&self) -> &str {
        self.code()
    }
}

pub fn builder(app: GuiApp) -> tauri::Builder<tauri::Wry> {
    let title = app.title;
    let html = app.html;

    tauri::Builder::default()
        .register_uri_scheme_protocol(GUI_PROTOCOL, move |_, request| {
            serve_html(&html, request.uri().path())
        })
        .setup(move |handle| {
            let url = custom_protocol_url();
            WebviewWindowBuilder::new(handle, "main", WebviewUrl::CustomProtocol(url))
                .title(title.clone())
                .inner_size(920.0, 720.0)
                .build()?;
            Ok(())
        })
}

pub fn run(builder: tauri::Builder<tauri::Wry>) -> Result<()> {
    builder
        .run(tauri::generate_context!())
        .context("Tauri GUI 运行失败")
}

pub fn command_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}

pub fn system_language() -> GuiLanguage {
    sys_locale::get_locale()
        .as_deref()
        .and_then(GuiLanguage::from_value)
        .unwrap_or(GuiLanguage::En)
}

pub fn language_for_qexed_config(config: &str) -> GuiLanguage {
    std::fs::read_to_string(config)
        .ok()
        .and_then(|content| content.parse::<toml::Value>().ok())
        .and_then(|value| {
            value
                .get("language")
                .and_then(toml::Value::as_str)
                .and_then(GuiLanguage::from_value)
        })
        .unwrap_or_else(system_language)
}

pub fn system_language_code() -> String {
    sys_locale::get_locale()
        .map(|language| normalize_language_code(&language))
        .unwrap_or_else(|| "en_us".to_string())
}

pub fn language_code_for_qexed_config(config: &str) -> String {
    std::fs::read_to_string(config)
        .ok()
        .and_then(|content| content.parse::<toml::Value>().ok())
        .and_then(|value| {
            value
                .get("language")
                .and_then(toml::Value::as_str)
                .map(normalize_language_code)
        })
        .unwrap_or_else(system_language_code)
}

pub fn normalize_language_code(language: &str) -> String {
    let normalized = language.trim().replace('-', "_").to_ascii_lowercase();
    match normalized.as_str() {
        "" => "en_us".to_string(),
        "en" => "en_us".to_string(),
        "zh" | "zh_cn" | "zh_hans" => "zh_cn".to_string(),
        other => other.to_string(),
    }
}

pub fn language_display_name(language: &str) -> &'static str {
    match normalize_language_code(language).as_str() {
        "af_za" => "Afrikaans",
        "ar_sa" => "العربية",
        "ast_es" => "Asturianu",
        "az_az" => "Azərbaycanca",
        "ba_ru" => "Башҡортса",
        "bar" => "Boarisch",
        "be_by" => "Беларуская",
        "be_latn" => "Biełaruskaja",
        "bg_bg" => "Български",
        "br_fr" => "Brezhoneg",
        "brb" => "Braobans",
        "bs_ba" => "Bosanski",
        "ca_es" => "Català",
        "cs_cz" => "Čeština",
        "cv_cu" => "Чӑвашла",
        "cy_gb" => "Cymraeg",
        "da_dk" => "Dansk",
        "de_at" => "Deitsch",
        "de_ch" => "Schwiizerdütsch",
        "de_de" => "Deutsch",
        "el_gr" => "Ελληνικά",
        "en_au" | "en_ca" | "en_gb" | "en_nz" | "en_us" => "English",
        "en_pt" => "Pirate Speak",
        "en_ud" => "ɥsᴉꞁᵷuƎ",
        "enp" => "Anglish",
        "enws" => "Shakespearean English",
        "eo_uy" => "Esperanto",
        "es_ar" | "es_cl" | "es_ec" | "es_es" | "es_mx" | "es_uy" | "es_ve" => "Español",
        "esan" => "Andalûh",
        "et_ee" => "Eesti keel",
        "eu_es" => "Euskara",
        "fa_ir" => "فارسی",
        "fi_fi" => "Suomi",
        "fil_ph" => "Filipino",
        "fo_fo" => "Føroyskt",
        "fr_ca" | "fr_ch" | "fr_fr" => "Français",
        "fra_de" => "Fränggisch",
        "fur_it" => "Furlan",
        "fy_nl" => "Frysk",
        "ga_ie" => "Gaeilge",
        "gd_gb" => "Gàidhlig",
        "gl_es" => "Galego",
        "go_fr" => "Galo",
        "hal_ua" => "Галицка",
        "haw_us" => "ʻŌlelo Hawaiʻi",
        "he_il" => "עברית",
        "hi_in" => "हिंदी",
        "hn_no" => "Høgnorsk",
        "hr_hr" => "Hrvatski",
        "hu_hu" => "Magyar",
        "hy_am" => "Հայերեն",
        "id_id" => "Bahasa Indonesia",
        "ig_ng" => "Igbo",
        "io_en" => "Ido",
        "is_is" => "Íslenska",
        "isv" => "Medžuslovjansky",
        "it_it" => "Italiano",
        "ja_jp" => "日本語",
        "jbo_en" => "la .lojban.",
        "ka_ge" => "ქართული",
        "kk_kz" => "Қазақша",
        "kn_in" => "ಕನ್ನಡ",
        "ko_kr" => "한국어",
        "ksh" => "Kölsch/Ripoarisch",
        "kw_gb" => "Kernewek",
        "ky_kg" => "Кыргызча",
        "la_la" => "Latina",
        "lb_lu" => "Lëtzebuergesch",
        "li_li" => "Limburgs",
        "lmo" => "Lombard",
        "lo_la" => "ລາວ",
        "lol_us" => "LOLCAT",
        "lt_lt" => "Lietuvių",
        "lv_lv" => "Latviešu",
        "lzh" => "文言",
        "mk_mk" => "Македонски",
        "mn_mn" => "Монгол",
        "ms_my" => "Bahasa Melayu",
        "mt_mt" => "Malti",
        "nah" => "Mēxikatlahtōlli",
        "nds_de" => "Plattdüütsch",
        "nl_be" => "Vlaams",
        "nl_nl" => "Nederlands",
        "nn_no" => "Norsk nynorsk",
        "no_no" => "Norsk bokmål",
        "oc_fr" => "Occitan",
        "ovd" => "Övdalska",
        "pl_pl" => "Polski",
        "pls" => "Ngiiwà",
        "pt_br" | "pt_pt" => "Português",
        "qcb_es" => "Cántabru/Montañés",
        "qid" => "Bhs. Indonesia edjaän lama",
        "qya_aa" => "Quenya",
        "ro_ro" => "Română",
        "rpr" => "Русскій дореформенный",
        "ru_ru" => "Русский",
        "ry_ua" => "Руснацькый",
        "sah_sah" => "Сахалыы",
        "se_no" => "Davvisámegiella",
        "sk_sk" => "Slovenčina",
        "sl_si" => "Slovenščina",
        "so_so" => "Soomaali",
        "sq_al" => "Shqip",
        "sr_cs" => "Srpski",
        "sr_sp" => "Српски",
        "sv_se" => "Svenska",
        "sxu" => "Säggs’sch",
        "szl" => "Ślōnski",
        "ta_in" => "தமிழ்",
        "th_th" => "ไทย",
        "tl_ph" => "Tagalog",
        "tlh_aa" => "tlhIngan Hol",
        "tok" => "toki pona",
        "tr_tr" => "Türkçe",
        "tt_ru" => "Татарча",
        "tzo_mx" => "Bats'i k'op",
        "uk_ua" => "Українська",
        "uz_uz" => "O'zbekcha",
        "val_es" => "Català (Valencià)",
        "vec_it" => "Vèneto",
        "vi_vn" => "Tiếng Việt",
        "vp_vl" => "Viossa",
        "vro" => "Võro",
        "yi_de" => "ייִדיש",
        "yo_ng" => "Yorùbá",
        "zh_cn" => "简体中文",
        "zh_hk" | "zh_tw" => "繁體中文",
        "zlm_arab" => "بهاس ملايو",
        _ => "Unknown",
    }
}

pub fn language_display_code(language: &str) -> String {
    let normalized = normalize_language_code(language);
    if normalized == "en_us" {
        "en".to_string()
    } else {
        normalized.replace('_', "-")
    }
}

pub fn language_option_label(language: &str) -> String {
    let display_code = language_display_code(language);
    let display_name = language_display_name(language);
    if display_name == "Unknown" {
        display_code
    } else {
        format!("{display_code} {display_name}")
    }
}

pub fn tr(language: &str, key: &str) -> &'static str {
    match (
        GuiLanguage::from_value(language).unwrap_or(GuiLanguage::En),
        key,
    ) {
        (GuiLanguage::ZhCn, "language") => "界面语言",
        (GuiLanguage::ZhCn, "saved") => "已保存",
        (GuiLanguage::ZhCn, "config_written") => "已写入配置",
        (GuiLanguage::ZhCn, "plugin_installed") => "已安装",
        (GuiLanguage::ZhCn, "plugin_enabled") => "已启用",
        (GuiLanguage::ZhCn, "plugin_disabled") => "已禁用",
        (GuiLanguage::ZhCn, "plugin_removed") => "已删除",
        (GuiLanguage::ZhCn, "installed_to") => "已安装到",
        (GuiLanguage::ZhCn, "package_created") => "已创建",
        (GuiLanguage::ZhCn, "status") => "状态",
        (GuiLanguage::ZhCn, "stdout") => "标准输出",
        (GuiLanguage::ZhCn, "stderr") => "标准错误",
        (_, "language") => "Interface language",
        (_, "saved") => "Saved",
        (_, "config_written") => "Configuration updated",
        (_, "plugin_installed") => "Installed",
        (_, "plugin_enabled") => "Enabled",
        (_, "plugin_disabled") => "Disabled",
        (_, "plugin_removed") => "Removed",
        (_, "installed_to") => "Installed to",
        (_, "package_created") => "Created",
        (_, "status") => "Status",
        (_, "stdout") => "stdout",
        (_, "stderr") => "stderr",
        _ => "",
    }
}

pub fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub fn language_selector(language: GuiLanguage) -> String {
    let zh_selected = if language == GuiLanguage::ZhCn {
        " selected"
    } else {
        ""
    };
    let en_selected = if language == GuiLanguage::En {
        " selected"
    } else {
        ""
    };
    format!(
        r#"<section class="language-panel">
<label for="guiLanguage" data-i18n="language">{}</label>
<select id="guiLanguage">
<option value="zh_cn"{zh_selected}>zh-cn 简体中文</option>
<option value="en_us"{en_selected}>en English</option>
</select>
</section>"#,
        tr(language.code(), "language")
    )
}

pub fn language_selector_for_codes(language: &str, languages: &[String]) -> String {
    let current = normalize_language_code(language);
    let mut options = languages
        .iter()
        .map(|language| normalize_language_code(language))
        .collect::<std::collections::BTreeSet<_>>();
    options.insert(current.clone());

    let mut html = format!(
        r#"<section class="language-panel">
<label for="guiLanguage" data-i18n="language">{}</label>
<select id="guiLanguage">"#,
        tr(&current, "language")
    );

    for language in options {
        let selected = if language == current { " selected" } else { "" };
        let label = escape_html(&language_option_label(&language));
        let language = escape_html(&language);
        html.push_str(&format!(
            r#"<option value="{language}"{selected}>{label}</option>"#
        ));
    }

    html.push_str("</select>\n</section>");
    html
}

pub fn i18n_script(
    default_language: GuiLanguage,
    zh_messages: &[(&str, &str)],
    en_messages: &[(&str, &str)],
) -> String {
    i18n_script_for_code(default_language.code(), zh_messages, en_messages)
}

pub fn i18n_script_for_code(
    default_language: &str,
    zh_messages: &[(&str, &str)],
    en_messages: &[(&str, &str)],
) -> String {
    i18n_script_from_messages(
        default_language,
        &[("zh_cn", zh_messages), ("en_us", en_messages)],
    )
}

pub fn i18n_script_from_messages(
    default_language: &str,
    language_messages: &[(&str, &[(&str, &str)])],
) -> String {
    let messages = language_messages
        .iter()
        .map(|(language, messages)| {
            (
                normalize_language_code(language),
                message_map_for_code(language, messages),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let messages = serde_json::to_string(&messages).expect("GUI messages are serializable");
    let default_language = serde_json::to_string(&normalize_language_code(default_language))
        .expect("language code is serializable");

    format!(
        r#"<script>
(() => {{
  const guiMessages = {messages};
  const guiDefaultLanguage = {default_language};
  const guiLanguageFallbacks = {{
    ca: 'ca_es', cs: 'cs_cz', da: 'da_dk', de: 'de_de', el: 'el_gr',
    en: 'en_us', es: 'es_es', fi: 'fi_fi', fr: 'fr_fr', hu: 'hu_hu',
    it: 'it_it', ja: 'ja_jp', ko: 'ko_kr', nl: 'nl_nl', no: 'no_no',
    pl: 'pl_pl', pt: 'pt_br', ro: 'ro_ro', ru: 'ru_ru', sk: 'sk_sk',
    sv: 'sv_se', tr: 'tr_tr', uk: 'uk_ua', vi: 'vi_vn'
  }};
  function normalizeGuiLanguage(language) {{
    const normalized = String(language || '').trim().replaceAll('-', '_').toLowerCase();
    if (!normalized) return 'en_us';
    if (normalized === 'en') return 'en_us';
    if (normalized === 'zh' || normalized === 'zh_cn' || normalized === 'zh_hans') return 'zh_cn';
    return normalized;
  }}
  function guiMessageLanguage(language) {{
    const normalized = normalizeGuiLanguage(language);
    if (guiMessages[normalized]) return normalized;
    if (normalized === 'zh_hk' || normalized === 'zh_tw' || normalized === 'lzh') {{
      if (guiMessages.zh_tw) return 'zh_tw';
    }}
    if (normalized.startsWith('zh') && guiMessages.zh_cn) return 'zh_cn';
    const base = normalized.split('_')[0];
    const fallback = guiLanguageFallbacks[base];
    if (fallback && guiMessages[fallback]) return fallback;
    return normalized;
  }}
  function guiHtmlLanguage(language) {{
    return String(language || '').replaceAll('_', '-');
  }}
  window.guiCurrentLanguage = function() {{
    const select = document.getElementById('guiLanguage');
    return select ? select.value : guiDefaultLanguage;
  }};
  window.guiT = function(key) {{
    const language = window.guiCurrentLanguage();
    const current = guiMessages[language] || guiMessages[guiMessageLanguage(language)] || {{}};
    const fallback = guiMessages.en_us || {{}};
    return current[key] || fallback[key] || key;
  }};
  function setGuiWindowTitle(title) {{
    document.title = title;
    const tauriWindow = window.__TAURI__ && window.__TAURI__.window;
    if (!tauriWindow) return;
    try {{
      const currentWindow = typeof tauriWindow.getCurrentWindow === 'function'
        ? tauriWindow.getCurrentWindow()
        : tauriWindow.appWindow;
      if (!currentWindow || typeof currentWindow.setTitle !== 'function') return;
      const result = currentWindow.setTitle(title);
      if (result && typeof result.catch === 'function') {{
        result.catch(() => {{}});
      }}
    }} catch (_) {{}}
  }}
  window.applyGuiLanguage = function() {{
    const language = window.guiCurrentLanguage();
    document.documentElement.lang = guiHtmlLanguage(language);
    const title = window.guiT('title');
    if (title !== 'title') {{
      setGuiWindowTitle(title);
    }}
    document.querySelectorAll('[data-i18n]').forEach((element) => {{
      element.textContent = window.guiT(element.dataset.i18n);
    }});
    document.querySelectorAll('[data-i18n-placeholder]').forEach((element) => {{
      element.setAttribute('placeholder', window.guiT(element.dataset.i18nPlaceholder));
    }});
    document.dispatchEvent(new CustomEvent('gui-language-change', {{detail: {{language}}}}));
  }};
  document.addEventListener('DOMContentLoaded', () => {{
    const select = document.getElementById('guiLanguage');
    if (select) {{
      select.value = guiDefaultLanguage;
      select.addEventListener('change', window.applyGuiLanguage);
    }}
    window.applyGuiLanguage();
  }});
}})();
</script>"#
    )
}

fn message_map_for_code<'a>(
    language: &str,
    page_messages: &[(&'a str, &'a str)],
) -> BTreeMap<&'a str, &'a str> {
    let mut messages = BTreeMap::from([("language", tr(language, "language"))]);
    for (key, value) in page_messages {
        messages.insert(*key, *value);
    }
    messages
}

pub fn html_page(language: GuiLanguage, title: &str, body: &str) -> String {
    html_page_for_code(language.code(), title, body)
}

pub fn html_page_for_code(language: &str, title: &str, body: &str) -> String {
    let title = escape_html(title);
    let language = escape_html(&normalize_language_code(language).replace('_', "-"));
    format!(
        r#"<!doctype html>
<html lang="{language}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<style>
body{{font-family:system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;margin:0;background:#f7f7f4;color:#242424}}
main{{max-width:920px;margin:32px auto;padding:0 20px}}
h1{{font-size:28px;margin:0 0 20px}}
section{{background:#fff;border:1px solid #deded8;border-radius:8px;padding:18px;margin:14px 0}}
label{{display:block;font-weight:600;margin:12px 0 6px}}
input,textarea,select{{width:100%;box-sizing:border-box;border:1px solid #bbb;border-radius:6px;padding:10px;font:inherit}}
textarea{{min-height:180px}}
button{{border:0;border-radius:6px;background:#1f6feb;color:white;padding:10px 14px;font-weight:700;cursor:pointer}}
button.secondary{{background:#555}}
pre{{background:#f1f1ee;border-radius:6px;padding:12px;overflow:auto;white-space:pre-wrap}}
table{{border-collapse:collapse;width:100%}}td,th{{border-bottom:1px solid #e6e6e0;padding:8px;text-align:left}}
.row{{display:flex;gap:10px;flex-wrap:wrap}}.row>*{{flex:1 1 220px}}
.language-panel{{padding:12px 18px}}
</style>
</head>
<body><main>{body}</main></body>
</html>"#
    )
}

fn serve_html(html: &str, path: &str) -> Response<Vec<u8>> {
    let body = if matches!(path, "" | "/" | "/index.html") {
        html.as_bytes().to_vec()
    } else {
        b"Not Found".to_vec()
    };

    let mut builder = Response::builder().header(header::CONTENT_TYPE, "text/html; charset=utf-8");
    if !matches!(path, "" | "/" | "/index.html") {
        builder = builder.status(StatusCode::NOT_FOUND);
        builder = builder.header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    }

    builder
        .header("Content-Security-Policy", GUI_CSP)
        .body(body)
        .expect("valid GUI response")
}

fn custom_protocol_url() -> Url {
    #[cfg(any(windows, target_os = "android"))]
    {
        Url::parse(&format!("http://{GUI_PROTOCOL}.localhost/")).expect("valid custom protocol url")
    }

    #[cfg(not(any(windows, target_os = "android")))]
    {
        Url::parse(&format!("{GUI_PROTOCOL}://localhost/")).expect("valid custom protocol url")
    }
}
