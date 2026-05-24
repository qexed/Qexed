#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use clap::Parser;
use qexed_tools::gui::{
    GuiApp, command_error, escape_html, html_page_for_code, i18n_script_from_messages,
    language_code_for_qexed_config, language_selector_for_codes,
};
use std::collections::BTreeSet;

type Messages = &'static [(&'static str, &'static str)];

const EN_US_MESSAGES: Messages = &[
    ("language", "Interface language"),
    ("title", "Qexed Code of Conduct"),
    ("heading", "Code of Conduct"),
    ("config_path", "Config file path"),
    ("enabled", "Enable prompt"),
    ("content_language", "Text language (locale)"),
    (
        "content_language_placeholder",
        "For example en_us or zh_cn; any client locale is accepted",
    ),
    ("load", "Load"),
    ("conduct_text", "Source"),
    ("preview", "Preview"),
    ("save", "Save"),
    ("saved", "Saved"),
    ("colors", "Colors"),
    ("styles", "Styles"),
    ("reset", "Reset"),
    ("bold", "Bold"),
    ("italic", "Italic"),
    ("underline", "Underline"),
    ("strikethrough", "Strikethrough"),
    ("black", "Black"),
    ("dark_blue", "Dark Blue"),
    ("dark_green", "Dark Green"),
    ("dark_aqua", "Dark Aqua"),
    ("dark_red", "Dark Red"),
    ("dark_purple", "Purple"),
    ("gold", "Gold"),
    ("gray", "Gray"),
    ("dark_gray", "Dark Gray"),
    ("blue", "Blue"),
    ("green", "Green"),
    ("aqua", "Aqua"),
    ("red", "Red"),
    ("light_purple", "Light Purple"),
    ("yellow", "Yellow"),
    ("white", "White"),
];

const ZH_CN_MESSAGES: Messages = &[
    ("language", "界面语言"),
    ("title", "Qexed 入服准则"),
    ("heading", "入服准则编辑"),
    ("config_path", "配置文件路径"),
    ("enabled", "启用入服准则"),
    ("content_language", "文本语言 (locale)"),
    (
        "content_language_placeholder",
        "例如 en_us、zh_cn；可输入任意客户端语言代码",
    ),
    ("load", "加载"),
    ("conduct_text", "源码"),
    ("preview", "预览"),
    ("save", "保存"),
    ("saved", "已保存"),
    ("colors", "颜色"),
    ("styles", "样式"),
    ("reset", "重置"),
    ("bold", "加粗"),
    ("italic", "斜体"),
    ("underline", "下划线"),
    ("strikethrough", "删除线"),
    ("black", "黑"),
    ("dark_blue", "深蓝"),
    ("dark_green", "深绿"),
    ("dark_aqua", "深青"),
    ("dark_red", "深红"),
    ("dark_purple", "紫"),
    ("gold", "金"),
    ("gray", "灰"),
    ("dark_gray", "深灰"),
    ("blue", "蓝"),
    ("green", "绿"),
    ("aqua", "青"),
    ("red", "红"),
    ("light_purple", "粉紫"),
    ("yellow", "黄"),
    ("white", "白"),
];

const ZH_TW_MESSAGES: Messages = &[
    ("language", "介面語言"),
    ("title", "Qexed 入服準則"),
    ("heading", "入服準則編輯"),
    ("config_path", "設定檔路徑"),
    ("enabled", "啟用入服準則"),
    ("content_language", "文字語言 (locale)"),
    (
        "content_language_placeholder",
        "例如 en_us、zh_tw；可輸入任意用戶端語言代碼",
    ),
    ("load", "載入"),
    ("conduct_text", "原始碼"),
    ("preview", "預覽"),
    ("save", "儲存"),
    ("saved", "已儲存"),
    ("colors", "顏色"),
    ("styles", "樣式"),
    ("reset", "重設"),
    ("bold", "粗體"),
    ("italic", "斜體"),
    ("underline", "底線"),
    ("strikethrough", "刪除線"),
    ("black", "黑色"),
    ("dark_blue", "深藍"),
    ("dark_green", "深綠"),
    ("dark_aqua", "深青"),
    ("dark_red", "深紅"),
    ("dark_purple", "紫色"),
    ("gold", "金色"),
    ("gray", "灰色"),
    ("dark_gray", "深灰"),
    ("blue", "藍色"),
    ("green", "綠色"),
    ("aqua", "青色"),
    ("red", "紅色"),
    ("light_purple", "粉紫"),
    ("yellow", "黃色"),
    ("white", "白色"),
];

const JA_JP_MESSAGES: Messages = &[
    ("language", "表示言語"),
    ("title", "Qexed 行動規範"),
    ("heading", "行動規範の編集"),
    ("config_path", "設定ファイルのパス"),
    ("enabled", "行動規範を有効化"),
    ("content_language", "テキスト言語 (locale)"),
    (
        "content_language_placeholder",
        "例: en_us、ja_jp。任意のクライアント言語コードを入力できます",
    ),
    ("load", "読み込み"),
    ("conduct_text", "ソース"),
    ("preview", "プレビュー"),
    ("save", "保存"),
    ("saved", "保存しました"),
    ("colors", "色"),
    ("styles", "スタイル"),
    ("reset", "リセット"),
    ("bold", "太字"),
    ("italic", "斜体"),
    ("underline", "下線"),
    ("strikethrough", "取り消し線"),
    ("black", "黒"),
    ("dark_blue", "濃い青"),
    ("dark_green", "濃い緑"),
    ("dark_aqua", "濃い水色"),
    ("dark_red", "濃い赤"),
    ("dark_purple", "紫"),
    ("gold", "金"),
    ("gray", "灰色"),
    ("dark_gray", "濃い灰色"),
    ("blue", "青"),
    ("green", "緑"),
    ("aqua", "水色"),
    ("red", "赤"),
    ("light_purple", "薄紫"),
    ("yellow", "黄色"),
    ("white", "白"),
];

const KO_KR_MESSAGES: Messages = &[
    ("language", "인터페이스 언어"),
    ("title", "Qexed 행동 강령"),
    ("heading", "행동 강령 편집"),
    ("config_path", "구성 파일 경로"),
    ("enabled", "행동 강령 활성화"),
    ("content_language", "텍스트 언어 (locale)"),
    (
        "content_language_placeholder",
        "예: en_us, ko_kr; 모든 클라이언트 언어 코드를 입력할 수 있습니다",
    ),
    ("load", "불러오기"),
    ("conduct_text", "소스"),
    ("preview", "미리보기"),
    ("save", "저장"),
    ("saved", "저장됨"),
    ("colors", "색상"),
    ("styles", "스타일"),
    ("reset", "초기화"),
    ("bold", "굵게"),
    ("italic", "기울임"),
    ("underline", "밑줄"),
    ("strikethrough", "취소선"),
    ("black", "검정"),
    ("dark_blue", "어두운 파랑"),
    ("dark_green", "어두운 초록"),
    ("dark_aqua", "어두운 청록"),
    ("dark_red", "어두운 빨강"),
    ("dark_purple", "보라"),
    ("gold", "금색"),
    ("gray", "회색"),
    ("dark_gray", "어두운 회색"),
    ("blue", "파랑"),
    ("green", "초록"),
    ("aqua", "청록"),
    ("red", "빨강"),
    ("light_purple", "연보라"),
    ("yellow", "노랑"),
    ("white", "흰색"),
];

const FR_FR_MESSAGES: Messages = &[
    ("language", "Langue de l'interface"),
    ("title", "Code de conduite Qexed"),
    ("heading", "Modifier le code de conduite"),
    ("config_path", "Chemin du fichier de configuration"),
    ("enabled", "Activer l'invite"),
    ("content_language", "Langue du texte (locale)"),
    (
        "content_language_placeholder",
        "Par exemple en_us ou fr_fr; tout code de langue client est accepté",
    ),
    ("load", "Charger"),
    ("conduct_text", "Source"),
    ("preview", "Aperçu"),
    ("save", "Enregistrer"),
    ("saved", "Enregistré"),
    ("colors", "Couleurs"),
    ("styles", "Styles"),
    ("reset", "Réinitialiser"),
    ("bold", "Gras"),
    ("italic", "Italique"),
    ("underline", "Souligné"),
    ("strikethrough", "Barré"),
    ("black", "Noir"),
    ("dark_blue", "Bleu foncé"),
    ("dark_green", "Vert foncé"),
    ("dark_aqua", "Cyan foncé"),
    ("dark_red", "Rouge foncé"),
    ("dark_purple", "Violet"),
    ("gold", "Or"),
    ("gray", "Gris"),
    ("dark_gray", "Gris foncé"),
    ("blue", "Bleu"),
    ("green", "Vert"),
    ("aqua", "Cyan"),
    ("red", "Rouge"),
    ("light_purple", "Violet clair"),
    ("yellow", "Jaune"),
    ("white", "Blanc"),
];

const DE_DE_MESSAGES: Messages = &[
    ("language", "Sprache der Oberfläche"),
    ("title", "Qexed-Verhaltenskodex"),
    ("heading", "Verhaltenskodex bearbeiten"),
    ("config_path", "Pfad der Konfigurationsdatei"),
    ("enabled", "Hinweis aktivieren"),
    ("content_language", "Textsprache (locale)"),
    (
        "content_language_placeholder",
        "Zum Beispiel en_us oder de_de; jeder Client-Sprachcode ist möglich",
    ),
    ("load", "Laden"),
    ("conduct_text", "Quelle"),
    ("preview", "Vorschau"),
    ("save", "Speichern"),
    ("saved", "Gespeichert"),
    ("colors", "Farben"),
    ("styles", "Stile"),
    ("reset", "Zurücksetzen"),
    ("bold", "Fett"),
    ("italic", "Kursiv"),
    ("underline", "Unterstrichen"),
    ("strikethrough", "Durchgestrichen"),
    ("black", "Schwarz"),
    ("dark_blue", "Dunkelblau"),
    ("dark_green", "Dunkelgrün"),
    ("dark_aqua", "Dunkeltürkis"),
    ("dark_red", "Dunkelrot"),
    ("dark_purple", "Violett"),
    ("gold", "Gold"),
    ("gray", "Grau"),
    ("dark_gray", "Dunkelgrau"),
    ("blue", "Blau"),
    ("green", "Grün"),
    ("aqua", "Türkis"),
    ("red", "Rot"),
    ("light_purple", "Hellviolett"),
    ("yellow", "Gelb"),
    ("white", "Weiß"),
];

const ES_ES_MESSAGES: Messages = &[
    ("language", "Idioma de la interfaz"),
    ("title", "Código de conducta de Qexed"),
    ("heading", "Editar código de conducta"),
    ("config_path", "Ruta del archivo de configuración"),
    ("enabled", "Activar aviso"),
    ("content_language", "Idioma del texto (locale)"),
    (
        "content_language_placeholder",
        "Por ejemplo en_us o es_es; se acepta cualquier código de idioma del cliente",
    ),
    ("load", "Cargar"),
    ("conduct_text", "Fuente"),
    ("preview", "Vista previa"),
    ("save", "Guardar"),
    ("saved", "Guardado"),
    ("colors", "Colores"),
    ("styles", "Estilos"),
    ("reset", "Restablecer"),
    ("bold", "Negrita"),
    ("italic", "Cursiva"),
    ("underline", "Subrayado"),
    ("strikethrough", "Tachado"),
    ("black", "Negro"),
    ("dark_blue", "Azul oscuro"),
    ("dark_green", "Verde oscuro"),
    ("dark_aqua", "Cian oscuro"),
    ("dark_red", "Rojo oscuro"),
    ("dark_purple", "Morado"),
    ("gold", "Dorado"),
    ("gray", "Gris"),
    ("dark_gray", "Gris oscuro"),
    ("blue", "Azul"),
    ("green", "Verde"),
    ("aqua", "Cian"),
    ("red", "Rojo"),
    ("light_purple", "Morado claro"),
    ("yellow", "Amarillo"),
    ("white", "Blanco"),
];

const PT_BR_MESSAGES: Messages = &[
    ("language", "Idioma da interface"),
    ("title", "Código de Conduta do Qexed"),
    ("heading", "Editar Código de Conduta"),
    ("config_path", "Caminho do arquivo de configuração"),
    ("enabled", "Ativar aviso"),
    ("content_language", "Idioma do texto (locale)"),
    (
        "content_language_placeholder",
        "Por exemplo en_us ou pt_br; qualquer código de idioma do cliente é aceito",
    ),
    ("load", "Carregar"),
    ("conduct_text", "Fonte"),
    ("preview", "Prévia"),
    ("save", "Salvar"),
    ("saved", "Salvo"),
    ("colors", "Cores"),
    ("styles", "Estilos"),
    ("reset", "Redefinir"),
    ("bold", "Negrito"),
    ("italic", "Itálico"),
    ("underline", "Sublinhado"),
    ("strikethrough", "Tachado"),
    ("black", "Preto"),
    ("dark_blue", "Azul escuro"),
    ("dark_green", "Verde escuro"),
    ("dark_aqua", "Ciano escuro"),
    ("dark_red", "Vermelho escuro"),
    ("dark_purple", "Roxo"),
    ("gold", "Dourado"),
    ("gray", "Cinza"),
    ("dark_gray", "Cinza escuro"),
    ("blue", "Azul"),
    ("green", "Verde"),
    ("aqua", "Ciano"),
    ("red", "Vermelho"),
    ("light_purple", "Roxo claro"),
    ("yellow", "Amarelo"),
    ("white", "Branco"),
];

const RU_RU_MESSAGES: Messages = &[
    ("language", "Язык интерфейса"),
    ("title", "Кодекс поведения Qexed"),
    ("heading", "Редактирование кодекса поведения"),
    ("config_path", "Путь к файлу конфигурации"),
    ("enabled", "Включить запрос"),
    ("content_language", "Язык текста (locale)"),
    (
        "content_language_placeholder",
        "Например en_us или ru_ru; можно ввести любой код языка клиента",
    ),
    ("load", "Загрузить"),
    ("conduct_text", "Исходный текст"),
    ("preview", "Предпросмотр"),
    ("save", "Сохранить"),
    ("saved", "Сохранено"),
    ("colors", "Цвета"),
    ("styles", "Стили"),
    ("reset", "Сброс"),
    ("bold", "Жирный"),
    ("italic", "Курсив"),
    ("underline", "Подчёркивание"),
    ("strikethrough", "Зачёркивание"),
    ("black", "Чёрный"),
    ("dark_blue", "Тёмно-синий"),
    ("dark_green", "Тёмно-зелёный"),
    ("dark_aqua", "Тёмно-бирюзовый"),
    ("dark_red", "Тёмно-красный"),
    ("dark_purple", "Фиолетовый"),
    ("gold", "Золотой"),
    ("gray", "Серый"),
    ("dark_gray", "Тёмно-серый"),
    ("blue", "Синий"),
    ("green", "Зелёный"),
    ("aqua", "Бирюзовый"),
    ("red", "Красный"),
    ("light_purple", "Светло-фиолетовый"),
    ("yellow", "Жёлтый"),
    ("white", "Белый"),
];

const UI_MESSAGES: &[(&str, Messages)] = &[
    ("af_za", EN_US_MESSAGES),
    ("ar_sa", EN_US_MESSAGES),
    ("ast_es", ES_ES_MESSAGES),
    ("az_az", EN_US_MESSAGES),
    ("ba_ru", RU_RU_MESSAGES),
    ("bar", DE_DE_MESSAGES),
    ("be_by", RU_RU_MESSAGES),
    ("be_latn", RU_RU_MESSAGES),
    ("bg_bg", RU_RU_MESSAGES),
    ("br_fr", FR_FR_MESSAGES),
    ("brb", EN_US_MESSAGES),
    ("bs_ba", EN_US_MESSAGES),
    ("ca_es", ES_ES_MESSAGES),
    ("cs_cz", EN_US_MESSAGES),
    ("cv_cu", RU_RU_MESSAGES),
    ("cy_gb", EN_US_MESSAGES),
    ("da_dk", EN_US_MESSAGES),
    ("de_at", DE_DE_MESSAGES),
    ("de_ch", DE_DE_MESSAGES),
    ("de_de", DE_DE_MESSAGES),
    ("el_gr", EN_US_MESSAGES),
    ("en_au", EN_US_MESSAGES),
    ("en_ca", EN_US_MESSAGES),
    ("en_gb", EN_US_MESSAGES),
    ("en_nz", EN_US_MESSAGES),
    ("en_pt", EN_US_MESSAGES),
    ("en_ud", EN_US_MESSAGES),
    ("en_us", EN_US_MESSAGES),
    ("enp", EN_US_MESSAGES),
    ("enws", EN_US_MESSAGES),
    ("eo_uy", EN_US_MESSAGES),
    ("es_ar", ES_ES_MESSAGES),
    ("es_cl", ES_ES_MESSAGES),
    ("es_ec", ES_ES_MESSAGES),
    ("es_es", ES_ES_MESSAGES),
    ("es_mx", ES_ES_MESSAGES),
    ("es_uy", ES_ES_MESSAGES),
    ("es_ve", ES_ES_MESSAGES),
    ("esan", ES_ES_MESSAGES),
    ("et_ee", EN_US_MESSAGES),
    ("eu_es", ES_ES_MESSAGES),
    ("fa_ir", EN_US_MESSAGES),
    ("fi_fi", EN_US_MESSAGES),
    ("fil_ph", EN_US_MESSAGES),
    ("fo_fo", EN_US_MESSAGES),
    ("fr_ca", FR_FR_MESSAGES),
    ("fr_ch", FR_FR_MESSAGES),
    ("fr_fr", FR_FR_MESSAGES),
    ("fra_de", DE_DE_MESSAGES),
    ("fur_it", EN_US_MESSAGES),
    ("fy_nl", EN_US_MESSAGES),
    ("ga_ie", EN_US_MESSAGES),
    ("gd_gb", EN_US_MESSAGES),
    ("gl_es", ES_ES_MESSAGES),
    ("go_fr", FR_FR_MESSAGES),
    ("hal_ua", RU_RU_MESSAGES),
    ("haw_us", EN_US_MESSAGES),
    ("he_il", EN_US_MESSAGES),
    ("hi_in", EN_US_MESSAGES),
    ("hn_no", EN_US_MESSAGES),
    ("hr_hr", EN_US_MESSAGES),
    ("hu_hu", EN_US_MESSAGES),
    ("hy_am", EN_US_MESSAGES),
    ("id_id", EN_US_MESSAGES),
    ("ig_ng", EN_US_MESSAGES),
    ("io_en", EN_US_MESSAGES),
    ("is_is", EN_US_MESSAGES),
    ("isv", RU_RU_MESSAGES),
    ("it_it", EN_US_MESSAGES),
    ("ja_jp", JA_JP_MESSAGES),
    ("jbo_en", EN_US_MESSAGES),
    ("ka_ge", EN_US_MESSAGES),
    ("kk_kz", RU_RU_MESSAGES),
    ("kn_in", EN_US_MESSAGES),
    ("ko_kr", KO_KR_MESSAGES),
    ("ksh", DE_DE_MESSAGES),
    ("kw_gb", EN_US_MESSAGES),
    ("ky_kg", RU_RU_MESSAGES),
    ("la_la", EN_US_MESSAGES),
    ("lb_lu", DE_DE_MESSAGES),
    ("li_li", DE_DE_MESSAGES),
    ("lmo", EN_US_MESSAGES),
    ("lo_la", EN_US_MESSAGES),
    ("lol_us", EN_US_MESSAGES),
    ("lt_lt", EN_US_MESSAGES),
    ("lv_lv", EN_US_MESSAGES),
    ("lzh", ZH_TW_MESSAGES),
    ("mk_mk", RU_RU_MESSAGES),
    ("mn_mn", RU_RU_MESSAGES),
    ("ms_my", EN_US_MESSAGES),
    ("mt_mt", EN_US_MESSAGES),
    ("nah", ES_ES_MESSAGES),
    ("nds_de", DE_DE_MESSAGES),
    ("nl_be", EN_US_MESSAGES),
    ("nl_nl", EN_US_MESSAGES),
    ("nn_no", EN_US_MESSAGES),
    ("no_no", EN_US_MESSAGES),
    ("oc_fr", FR_FR_MESSAGES),
    ("ovd", EN_US_MESSAGES),
    ("pl_pl", EN_US_MESSAGES),
    ("pls", EN_US_MESSAGES),
    ("pt_br", PT_BR_MESSAGES),
    ("pt_pt", PT_BR_MESSAGES),
    ("qcb_es", ES_ES_MESSAGES),
    ("qid", EN_US_MESSAGES),
    ("qya_aa", EN_US_MESSAGES),
    ("ro_ro", EN_US_MESSAGES),
    ("rpr", RU_RU_MESSAGES),
    ("ru_ru", RU_RU_MESSAGES),
    ("ry_ua", RU_RU_MESSAGES),
    ("sah_sah", RU_RU_MESSAGES),
    ("se_no", EN_US_MESSAGES),
    ("sk_sk", EN_US_MESSAGES),
    ("sl_si", EN_US_MESSAGES),
    ("so_so", EN_US_MESSAGES),
    ("sq_al", EN_US_MESSAGES),
    ("sr_cs", RU_RU_MESSAGES),
    ("sr_sp", RU_RU_MESSAGES),
    ("sv_se", EN_US_MESSAGES),
    ("sxu", DE_DE_MESSAGES),
    ("szl", EN_US_MESSAGES),
    ("ta_in", EN_US_MESSAGES),
    ("th_th", EN_US_MESSAGES),
    ("tl_ph", EN_US_MESSAGES),
    ("tlh_aa", EN_US_MESSAGES),
    ("tok", EN_US_MESSAGES),
    ("tr_tr", EN_US_MESSAGES),
    ("tt_ru", RU_RU_MESSAGES),
    ("tzo_mx", ES_ES_MESSAGES),
    ("uk_ua", RU_RU_MESSAGES),
    ("uz_uz", EN_US_MESSAGES),
    ("val_es", ES_ES_MESSAGES),
    ("vec_it", EN_US_MESSAGES),
    ("vi_vn", EN_US_MESSAGES),
    ("vp_vl", EN_US_MESSAGES),
    ("vro", EN_US_MESSAGES),
    ("yi_de", DE_DE_MESSAGES),
    ("yo_ng", EN_US_MESSAGES),
    ("zh_cn", ZH_CN_MESSAGES),
    ("zh_hk", ZH_TW_MESSAGES),
    ("zh_tw", ZH_TW_MESSAGES),
    ("zlm_arab", EN_US_MESSAGES),
];

#[derive(Debug, Parser)]
#[command(name = "qexed_code_of_conduct_gui")]
struct Args {
    #[arg(long, default_value = "config/qexed.toml")]
    config: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeOfConductState {
    enabled: bool,
    content_language: String,
    content_languages: Vec<String>,
    text: String,
}

#[tauri::command]
fn load_code_of_conduct(
    config: String,
    content_language: String,
) -> Result<CodeOfConductState, String> {
    let content_language =
        qexed_tools::code_of_conduct::normalize_language_file_name(&content_language);
    let enabled = qexed_tools::code_of_conduct::is_enabled(&config).map_err(command_error)?;
    let text = qexed_tools::code_of_conduct::read_language(&config, &content_language)
        .map_err(command_error)?;
    let content_languages = content_language_options(&config, &content_language);
    Ok(CodeOfConductState {
        enabled,
        content_language,
        content_languages,
        text,
    })
}

#[tauri::command]
fn save_code_of_conduct(
    config: String,
    text: String,
    content_language: String,
    enabled: bool,
    language: String,
) -> Result<String, String> {
    qexed_tools::code_of_conduct::save(&config, &content_language, &text, enabled)
        .map_err(command_error)?;
    Ok(ui_text(&language, "saved").to_string())
}

fn ui_messages_for(language: &str) -> Messages {
    let normalized = qexed_tools::gui::normalize_language_code(language);
    match normalized.as_str() {
        "zh_hk" | "zh_tw" | "lzh" => return ZH_TW_MESSAGES,
        "zh_cn" => return ZH_CN_MESSAGES,
        _ => {}
    }

    if let Some((_, messages)) = UI_MESSAGES
        .iter()
        .find(|(candidate, _)| *candidate == normalized)
    {
        return messages;
    }

    let base = normalized.split('_').next().unwrap_or_default();
    match base {
        "de" => DE_DE_MESSAGES,
        "en" => EN_US_MESSAGES,
        "es" => ES_ES_MESSAGES,
        "fr" => FR_FR_MESSAGES,
        "ja" => JA_JP_MESSAGES,
        "ko" => KO_KR_MESSAGES,
        "pt" => PT_BR_MESSAGES,
        "ru" => RU_RU_MESSAGES,
        "zh" => ZH_CN_MESSAGES,
        _ => EN_US_MESSAGES,
    }
}

fn all_ui_messages() -> Vec<(&'static str, Messages)> {
    UI_MESSAGES.to_vec()
}

fn ui_text(language: &str, key: &'static str) -> &'static str {
    ui_messages_for(language)
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, value)| *value)
        .or_else(|| {
            EN_US_MESSAGES
                .iter()
                .find(|(candidate, _)| *candidate == key)
                .map(|(_, value)| *value)
        })
        .unwrap_or(key)
}

fn content_language_options(config: &str, selected: &str) -> Vec<String> {
    let mut languages = qexed_tools::code_of_conduct::MINECRAFT_LANGUAGE_CODES
        .iter()
        .map(|language| (*language).to_string())
        .collect::<BTreeSet<_>>();
    languages.insert(qexed_tools::code_of_conduct::normalize_language_file_name(
        selected,
    ));

    if let Ok(existing) = qexed_tools::code_of_conduct::language_files(config) {
        languages.extend(existing);
    }

    languages.into_iter().collect()
}

fn interface_language_options(selected: &str) -> Vec<String> {
    let mut languages = qexed_tools::code_of_conduct::MINECRAFT_LANGUAGE_CODES
        .iter()
        .map(|language| (*language).to_string())
        .collect::<BTreeSet<_>>();
    languages.insert(qexed_tools::gui::normalize_language_code(selected));
    languages.into_iter().collect()
}

fn main() -> Result<()> {
    let args = Args::parse();
    let language = language_code_for_qexed_config(&args.config);
    let content_language = qexed_tools::code_of_conduct::default_language_for_config(&args.config);
    let enabled = qexed_tools::code_of_conduct::is_enabled(&args.config).unwrap_or(false);
    let current = qexed_tools::code_of_conduct::read_language(&args.config, &content_language)
        .unwrap_or_default();
    let content_languages =
        serde_json::to_string(&content_language_options(&args.config, &content_language))?;
    let interface_languages = interface_language_options(&language);
    let page_title = ui_text(&language, "title");
    let config = escape_html(&args.config);
    let content_language = escape_html(&content_language);
    let enabled_checked = if enabled { " checked" } else { "" };
    let current = escape_html(&current);
    let ui_messages = all_ui_messages();
    let i18n = i18n_script_from_messages(&language, &ui_messages);
    let html = html_page_for_code(
        &language,
        page_title,
        &format!(
            r#"{}
<h1 data-i18n="heading"></h1>
<section>
<label data-i18n="config_path"></label>
<input id="config" value="{config}">
<div class="row conduct-options">
  <div>
    <label data-i18n="content_language"></label>
    <input id="contentLanguage" value="{content_language}" list="contentLanguageOptions" spellcheck="false" autocapitalize="none" data-i18n-placeholder="content_language_placeholder">
    <datalist id="contentLanguageOptions"></datalist>
  </div>
  <label class="check-option">
    <input id="enabled" type="checkbox"{enabled_checked}>
    <span data-i18n="enabled"></span>
  </label>
</div>
<button type="button" class="secondary" onclick="loadText()" data-i18n="load"></button>
<div class="editor-grid">
  <div>
    <label data-i18n="conduct_text"></label>
    <div class="toolbar">
      <div class="tool-group">
        <span data-i18n="colors"></span>
        <button type="button" class="swatch mc-black" onclick="applyFormat('0')" data-i18n-title="black" title="Black"></button>
        <button type="button" class="swatch mc-dark-blue" onclick="applyFormat('1')" data-i18n-title="dark_blue" title="Dark Blue"></button>
        <button type="button" class="swatch mc-dark-green" onclick="applyFormat('2')" data-i18n-title="dark_green" title="Dark Green"></button>
        <button type="button" class="swatch mc-dark-aqua" onclick="applyFormat('3')" data-i18n-title="dark_aqua" title="Dark Aqua"></button>
        <button type="button" class="swatch mc-dark-red" onclick="applyFormat('4')" data-i18n-title="dark_red" title="Dark Red"></button>
        <button type="button" class="swatch mc-dark-purple" onclick="applyFormat('5')" data-i18n-title="dark_purple" title="Purple"></button>
        <button type="button" class="swatch mc-gold" onclick="applyFormat('6')" data-i18n-title="gold" title="Gold"></button>
        <button type="button" class="swatch mc-gray" onclick="applyFormat('7')" data-i18n-title="gray" title="Gray"></button>
        <button type="button" class="swatch mc-dark-gray" onclick="applyFormat('8')" data-i18n-title="dark_gray" title="Dark Gray"></button>
        <button type="button" class="swatch mc-blue" onclick="applyFormat('9')" data-i18n-title="blue" title="Blue"></button>
        <button type="button" class="swatch mc-green" onclick="applyFormat('a')" data-i18n-title="green" title="Green"></button>
        <button type="button" class="swatch mc-aqua" onclick="applyFormat('b')" data-i18n-title="aqua" title="Aqua"></button>
        <button type="button" class="swatch mc-red" onclick="applyFormat('c')" data-i18n-title="red" title="Red"></button>
        <button type="button" class="swatch mc-light-purple" onclick="applyFormat('d')" data-i18n-title="light_purple" title="Light Purple"></button>
        <button type="button" class="swatch mc-yellow" onclick="applyFormat('e')" data-i18n-title="yellow" title="Yellow"></button>
        <button type="button" class="swatch mc-white" onclick="applyFormat('f')" data-i18n-title="white" title="White"></button>
      </div>
      <div class="tool-group">
        <span data-i18n="styles"></span>
        <button type="button" class="tool-button" onclick="applyFormat('l')" data-i18n="bold"></button>
        <button type="button" class="tool-button" onclick="applyFormat('o')" data-i18n="italic"></button>
        <button type="button" class="tool-button" onclick="applyFormat('n')" data-i18n="underline"></button>
        <button type="button" class="tool-button" onclick="applyFormat('m')" data-i18n="strikethrough"></button>
        <button type="button" class="tool-button secondary" onclick="applyFormat('r')" data-i18n="reset"></button>
      </div>
    </div>
    <textarea id="text" spellcheck="false">{current}</textarea>
  </div>
  <div>
    <label data-i18n="preview"></label>
    <div id="preview" class="preview"></div>
  </div>
</div>
<button onclick="save()" data-i18n="save"></button>
<pre id="out"></pre>
</section>
<style>
.editor-grid{{display:grid;grid-template-columns:minmax(0,1fr) minmax(280px,0.8fr);gap:18px;align-items:start}}
.conduct-options{{align-items:end;margin:10px 0}}
.check-option{{display:flex;align-items:center;gap:8px;margin:0 0 6px;font-weight:700}}
.check-option input{{width:auto}}
.toolbar{{border:1px solid #deded8;border-radius:8px;background:#fafaf7;padding:10px;margin:0 0 10px}}
.tool-group{{display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin:6px 0}}
.tool-group>span{{font-weight:700;margin-right:4px}}
.swatch{{width:28px;height:28px;min-width:28px;border:1px solid rgba(0,0,0,.25);border-radius:6px;padding:0}}
.tool-button{{padding:7px 10px}}
#text{{min-height:360px;font-family:Consolas,"Cascadia Mono",monospace;line-height:1.45}}
.preview{{min-height:360px;border:1px solid #bbb;border-radius:6px;background:#2b2b2b;color:#fff;padding:12px;white-space:pre-wrap;overflow:auto;line-height:1.45;font-family:system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
.mc-black{{background:#000000}}.mc-dark-blue{{background:#0000aa}}.mc-dark-green{{background:#00aa00}}.mc-dark-aqua{{background:#00aaaa}}.mc-dark-red{{background:#aa0000}}.mc-dark-purple{{background:#aa00aa}}.mc-gold{{background:#ffaa00}}.mc-gray{{background:#aaaaaa}}.mc-dark-gray{{background:#555555}}.mc-blue{{background:#5555ff}}.mc-green{{background:#55ff55}}.mc-aqua{{background:#55ffff}}.mc-red{{background:#ff5555}}.mc-light-purple{{background:#ff55ff}}.mc-yellow{{background:#ffff55}}.mc-white{{background:#ffffff}}
@media (max-width:760px){{.editor-grid{{grid-template-columns:1fr}}}}
</style>
{i18n}
<script>
const invoke = window.__TAURI__.core.invoke;
let knownContentLanguages = {content_languages};
const colorCodes = {{
  '0': '#000000', '1': '#0000aa', '2': '#00aa00', '3': '#00aaaa',
  '4': '#aa0000', '5': '#aa00aa', '6': '#ffaa00', '7': '#aaaaaa',
  '8': '#555555', '9': '#5555ff', 'a': '#55ff55', 'b': '#55ffff',
  'c': '#ff5555', 'd': '#ff55ff', 'e': '#ffff55', 'f': '#ffffff'
}};
const colorCodeSet = new Set(Object.keys(colorCodes));
function applyFormat(code) {{
  const start = text.selectionStart;
  const end = text.selectionEnd;
  const selected = text.value.slice(start, end);
  const prefix = '§' + code;
  const replacement = selected ? prefix + selected + '§r' : prefix;
  text.setRangeText(replacement, start, end, 'end');
  text.focus();
  renderPreview();
}}
function resetStyle() {{
  return {{color: '#ffffff', bold: false, italic: false, underline: false, strikethrough: false}};
}}
function appendStyled(container, value, style) {{
  if (!value) return;
  const span = document.createElement('span');
  span.textContent = value;
  span.style.color = style.color;
  span.style.fontWeight = style.bold ? '700' : '400';
  span.style.fontStyle = style.italic ? 'italic' : 'normal';
  const decorations = [];
  if (style.underline) decorations.push('underline');
  if (style.strikethrough) decorations.push('line-through');
  span.style.textDecoration = decorations.join(' ');
  container.appendChild(span);
}}
function renderMinecraftText(value, container) {{
  container.textContent = '';
  let style = resetStyle();
  let buffer = '';
  for (let i = 0; i < value.length; i++) {{
    const ch = value[i];
    if (ch === '§' && i + 1 < value.length) {{
      const code = value[++i].toLowerCase();
      appendStyled(container, buffer, style);
      buffer = '';
      if (colorCodeSet.has(code)) {{
        style = {{...resetStyle(), color: colorCodes[code]}};
      }} else if (code === 'l') {{
        style.bold = true;
      }} else if (code === 'o') {{
        style.italic = true;
      }} else if (code === 'n') {{
        style.underline = true;
      }} else if (code === 'm') {{
        style.strikethrough = true;
      }} else if (code === 'r') {{
        style = resetStyle();
      }}
    }} else {{
      buffer += ch;
    }}
  }}
  appendStyled(container, buffer, style);
}}
function renderPreview() {{
  renderMinecraftText(text.value, preview);
}}
function normalizeContentLanguage(value) {{
  const normalized = value.trim().replaceAll('-', '_').toLowerCase();
  if (!normalized) return 'en_us';
  if (normalized === 'en') return 'en_us';
  if (normalized === 'zh' || normalized === 'zh_cn' || normalized === 'zh_hans') return 'zh_cn';
  return normalized;
}}
function renderContentLanguageOptions(languages = knownContentLanguages) {{
  const next = new Set(languages || []);
  next.add(normalizeContentLanguage(contentLanguage.value));
  knownContentLanguages = [...next].filter(Boolean).sort();
  contentLanguageOptions.textContent = '';
  knownContentLanguages.forEach((language) => {{
    const option = document.createElement('option');
    option.value = language;
    contentLanguageOptions.appendChild(option);
  }});
}}
async function loadText() {{
  try {{
    contentLanguage.value = normalizeContentLanguage(contentLanguage.value);
    const state = await invoke('load_code_of_conduct', {{config: config.value, contentLanguage: contentLanguage.value}});
    contentLanguage.value = state.contentLanguage;
    renderContentLanguageOptions(state.contentLanguages);
    enabled.checked = state.enabled;
    text.value = state.text;
    out.textContent = '';
    renderPreview();
  }} catch (error) {{
    out.textContent = String(error);
  }}
}}
async function save() {{
  try {{
    contentLanguage.value = normalizeContentLanguage(contentLanguage.value);
    renderContentLanguageOptions([...knownContentLanguages, contentLanguage.value]);
    out.textContent = await invoke('save_code_of_conduct', {{config: config.value, text: text.value, contentLanguage: contentLanguage.value, enabled: enabled.checked, language: guiCurrentLanguage()}});
  }} catch (error) {{
    out.textContent = String(error);
  }}
}}
document.addEventListener('DOMContentLoaded', () => {{
  renderContentLanguageOptions();
  text.addEventListener('input', renderPreview);
  contentLanguage.addEventListener('change', loadText);
  renderPreview();
}});
document.addEventListener('gui-language-change', () => {{
  document.querySelectorAll('[data-i18n-title]').forEach((element) => {{
    element.title = guiT(element.dataset.i18nTitle);
  }});
}});
</script>"#,
            language_selector_for_codes(&language, &interface_languages)
        ),
    );

    let app = GuiApp {
        title: page_title.to_string(),
        html,
    };
    let builder = qexed_tools::gui::builder(app).invoke_handler(tauri::generate_handler![
        load_code_of_conduct,
        save_code_of_conduct
    ]);
    qexed_tools::gui::run(builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn registers_ui_messages_for_every_minecraft_language() {
        let messages = all_ui_messages();
        assert_eq!(
            messages.len(),
            qexed_tools::code_of_conduct::MINECRAFT_LANGUAGE_CODES.len()
        );
        let registered = messages
            .iter()
            .map(|(language, _)| *language)
            .collect::<BTreeSet<_>>();
        let expected = qexed_tools::code_of_conduct::MINECRAFT_LANGUAGE_CODES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        assert_eq!(registered, expected);
        assert!(messages.iter().all(|(_, messages)| {
            messages.iter().any(|(key, _)| *key == "title")
                && messages.iter().any(|(key, _)| *key == "saved")
        }));
    }
}
