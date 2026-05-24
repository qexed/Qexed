use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, value};

const DIR_NAME: &str = "enable-code-of-conduct";

pub const MINECRAFT_LANGUAGE_CODES: &[&str] = &[
    "af_za", "ar_sa", "ast_es", "az_az", "ba_ru", "bar", "be_by", "be_latn", "bg_bg", "br_fr",
    "brb", "bs_ba", "ca_es", "cs_cz", "cv_cu", "cy_gb", "da_dk", "de_at", "de_ch", "de_de",
    "el_gr", "en_au", "en_ca", "en_gb", "en_nz", "en_pt", "en_ud", "en_us", "enp", "enws", "eo_uy",
    "es_ar", "es_cl", "es_ec", "es_es", "es_mx", "es_uy", "es_ve", "esan", "et_ee", "eu_es",
    "fa_ir", "fi_fi", "fil_ph", "fo_fo", "fr_ca", "fr_ch", "fr_fr", "fra_de", "fur_it", "fy_nl",
    "ga_ie", "gd_gb", "gl_es", "go_fr", "hal_ua", "haw_us", "he_il", "hi_in", "hn_no", "hr_hr",
    "hu_hu", "hy_am", "id_id", "ig_ng", "io_en", "is_is", "isv", "it_it", "ja_jp", "jbo_en",
    "ka_ge", "kk_kz", "kn_in", "ko_kr", "ksh", "kw_gb", "ky_kg", "la_la", "lb_lu", "li_li", "lmo",
    "lo_la", "lol_us", "lt_lt", "lv_lv", "lzh", "mk_mk", "mn_mn", "ms_my", "mt_mt", "nah",
    "nds_de", "nl_be", "nl_nl", "nn_no", "no_no", "oc_fr", "ovd", "pl_pl", "pls", "pt_br", "pt_pt",
    "qcb_es", "qid", "qya_aa", "ro_ro", "rpr", "ru_ru", "ry_ua", "sah_sah", "se_no", "sk_sk",
    "sl_si", "so_so", "sq_al", "sr_cs", "sr_sp", "sv_se", "sxu", "szl", "ta_in", "th_th", "tl_ph",
    "tlh_aa", "tok", "tr_tr", "tt_ru", "tzo_mx", "uk_ua", "uz_uz", "val_es", "vec_it", "vi_vn",
    "vp_vl", "vro", "yi_de", "yo_ng", "zh_cn", "zh_hk", "zh_tw", "zlm_arab",
];

pub fn read(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let language = default_language_for_config(path);
    read_language(path, &language)
}

pub fn read_language(path: impl AsRef<Path>, language: &str) -> Result<String> {
    let path = path.as_ref();
    if let Some(file) = find_text_file(path, language)? {
        return read_text_file(&file);
    }

    if normalize_language_file_name(language) != "en_us" {
        if let Some(file) = find_text_file(path, "en_us")? {
            return read_text_file(&file);
        }
    }

    if let Some(file) = first_text_file(path)? {
        return read_text_file(&file);
    }

    read_legacy_inline_text(path)
}

pub fn write(path: impl AsRef<Path>, text: &str) -> Result<()> {
    let path = path.as_ref();
    let language = default_language_for_config(path);
    save(path, &language, text, true)
}

pub fn save(path: impl AsRef<Path>, language: &str, text: &str, enabled: bool) -> Result<()> {
    let path = path.as_ref();
    set_enabled(path, enabled)?;

    let dir = text_dir(path);
    fs::create_dir_all(&dir).with_context(|| format!("无法创建入服准则目录 {}", dir.display()))?;
    let file = text_file(path, language);
    fs::write(&file, text).with_context(|| format!("无法写入入服准则文件 {}", file.display()))
}

pub fn language_files(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let dir = text_dir(path.as_ref());
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut languages = BTreeSet::new();
    for entry in
        fs::read_dir(&dir).with_context(|| format!("无法读取入服准则目录 {}", dir.display()))?
    {
        let path = entry?.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
        {
            let language = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(normalize_language_file_name)
                .unwrap_or_default();
            if !language.is_empty() {
                languages.insert(language);
            }
        }
    }

    Ok(languages.into_iter().collect())
}

pub fn is_enabled(path: impl AsRef<Path>) -> Result<bool> {
    let doc = read_config_doc(path.as_ref())?;
    Ok(doc
        .get("server")
        .and_then(|server| server.get("code_of_conduct"))
        .and_then(|item| {
            item.as_bool()
                .or_else(|| item.as_str().map(|text| !text.trim().is_empty()))
        })
        .unwrap_or(false))
}

pub fn set_enabled(path: impl AsRef<Path>, enabled: bool) -> Result<()> {
    let path = path.as_ref();
    let mut doc = read_config_doc(path)?;
    let server = doc["server"]
        .as_table_mut()
        .context("配置文件缺少 [server] 表")?;
    server.insert("code_of_conduct", value(enabled));
    fs::write(path, doc.to_string()).with_context(|| format!("无法写入配置文件 {}", path.display()))
}

pub fn default_language_for_config(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .ok()
        .and_then(|content| content.parse::<toml::Value>().ok())
        .and_then(|value| {
            value
                .get("language")
                .and_then(toml::Value::as_str)
                .map(normalize_language_file_name)
        })
        .or_else(|| {
            sys_locale::get_locale().map(|language| normalize_language_file_name(&language))
        })
        .unwrap_or_else(|| "en_us".to_string())
}

pub fn normalize_language_file_name(language: &str) -> String {
    let normalized = language.trim().replace('-', "_").to_ascii_lowercase();
    match normalized.as_str() {
        "" => "en_us".to_string(),
        "en" => "en_us".to_string(),
        "zh" | "zh_cn" | "zh_hans" => "zh_cn".to_string(),
        other => other.to_string(),
    }
}

fn read_config_doc(path: &Path) -> Result<DocumentMut> {
    let content =
        fs::read_to_string(path).with_context(|| format!("无法读取配置文件 {}", path.display()))?;
    content
        .parse::<DocumentMut>()
        .with_context(|| format!("配置文件不是合法 TOML: {}", path.display()))
}

fn read_legacy_inline_text(path: &Path) -> Result<String> {
    let doc = read_config_doc(path)?;
    Ok(doc
        .get("server")
        .and_then(|server| server.get("code_of_conduct"))
        .and_then(|item| item.as_str())
        .unwrap_or_default()
        .to_string())
}

fn read_text_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("无法读取入服准则文件 {}", path.display()))
}

fn first_text_file(path: &Path) -> Result<Option<PathBuf>> {
    let dir = text_dir(path);
    if !dir.is_dir() {
        return Ok(None);
    }

    let mut files = Vec::new();
    for entry in
        fs::read_dir(&dir).with_context(|| format!("无法读取入服准则目录 {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files.into_iter().next())
}

fn find_text_file(path: &Path, language: &str) -> Result<Option<PathBuf>> {
    let dir = text_dir(path);
    if !dir.is_dir() {
        return Ok(None);
    }

    let wanted = normalize_language_file_name(language);
    for entry in
        fs::read_dir(&dir).with_context(|| format!("无法读取入服准则目录 {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
        {
            let current = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(normalize_language_file_name)
                .unwrap_or_default();
            if current == wanted {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}

fn text_dir(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(DIR_NAME)
}

fn text_file(path: &Path, language: &str) -> PathBuf {
    text_dir(path).join(format!("{}.txt", normalize_language_file_name(language)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legacy_inline_text_and_writes_language_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qexed.toml");
        fs::write(
            &path,
            "language = \"zh-CN\"\n[server]\ncode_of_conduct = \"old\"\n",
        )
        .unwrap();

        assert!(is_enabled(&path).unwrap());
        assert_eq!(default_language_for_config(&path), "zh_cn");
        assert_eq!(read(&path).unwrap(), "old");

        save(&path, "zh_cn", "\u{00a7}cnew", true).unwrap();

        assert!(is_enabled(&path).unwrap());
        assert_eq!(read_language(&path, "zh-CN").unwrap(), "\u{00a7}cnew");
        let saved_config = fs::read_to_string(&path).unwrap();
        assert!(saved_config.contains("code_of_conduct = true"));
    }

    #[test]
    fn can_disable_prompt_without_removing_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qexed.toml");
        fs::write(&path, "[server]\ncode_of_conduct = true\n").unwrap();

        save(&path, "en_us", "rules", false).unwrap();

        assert!(!is_enabled(&path).unwrap());
        assert_eq!(read_language(&path, "en").unwrap(), "rules");
    }

    #[test]
    fn lists_existing_language_files_as_normalized_locale_codes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qexed.toml");
        fs::write(&path, "[server]\ncode_of_conduct = true\n").unwrap();
        fs::create_dir(dir.path().join(DIR_NAME)).unwrap();
        fs::write(dir.path().join(DIR_NAME).join("zh-CN.txt"), "rules").unwrap();
        fs::write(dir.path().join(DIR_NAME).join("en_us.txt"), "rules").unwrap();
        fs::write(dir.path().join(DIR_NAME).join("readme.md"), "ignored").unwrap();

        assert_eq!(language_files(&path).unwrap(), vec!["en_us", "zh_cn"]);
    }

    #[test]
    fn known_minecraft_language_codes_cover_non_default_client_locales() {
        assert!(MINECRAFT_LANGUAGE_CODES.contains(&"en_us"));
        assert!(MINECRAFT_LANGUAGE_CODES.contains(&"fr_fr"));
        assert!(MINECRAFT_LANGUAGE_CODES.contains(&"ja_jp"));
        assert!(MINECRAFT_LANGUAGE_CODES.contains(&"zh_tw"));
    }
}
