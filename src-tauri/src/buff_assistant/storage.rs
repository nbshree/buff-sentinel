use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{DynamicImage, GrayImage};
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use super::{
    audio,
    detector::TemplateData,
    model::{
        BuffAssistantConfig, BuffCustomSoundAsset, BuffSoundCue, BuffSoundSource,
        BuffSoundTemplateSummary, BuffTemplateSummary,
    },
};

const CONFIG_FILE: &str = "config-v1.json";
const SOUND_ASSETS_DIRECTORY: &str = "sound-assets";
const MAX_SOUND_FILE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Clone)]
pub struct SoundTemplate {
    pub summary: BuffSoundTemplateSummary,
    directory: PathBuf,
    files: SoundTemplateFiles,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoundTemplateFiles {
    triggered: String,
    prewarn_three: String,
    prewarn_two: String,
    prewarn_one: String,
}

#[derive(Deserialize)]
struct SoundTemplateManifest {
    id: String,
    name: String,
    files: SoundTemplateFiles,
}

pub fn storage_directory(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("buff-assistant"))
        .map_err(|error| format!("无法确定 Buff 助手配置目录：{error}"))
}

pub fn sound_templates_directory(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resource_dir()
        .map(|directory| directory.join("buff-sounds"))
        .map_err(|error| format!("无法确定提示音模板目录：{error}"))
}

pub fn load_config(directory: &Path) -> (BuffAssistantConfig, Vec<String>) {
    let mut notices = Vec::new();
    let path = directory.join(CONFIG_FILE);
    let mut config = if path.exists() {
        fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|contents| {
                serde_json::from_str::<BuffAssistantConfig>(&contents)
                    .map_err(|error| error.to_string())
            })
            .unwrap_or_else(|error| {
                notices.push(format!("Buff 助手配置读取失败，已使用默认配置：{error}"));
                BuffAssistantConfig::default()
            })
    } else {
        BuffAssistantConfig::default()
    };
    config.sanitize();
    if config
        .template
        .as_ref()
        .is_some_and(|template| !template_directory(directory, &template.id).exists())
    {
        notices.push("Buff 图标模板文件不存在，请重新配置".into());
        config.template = None;
    }
    cleanup_unused_sound_assets(directory, &config);
    (config, notices)
}

pub fn save_config(directory: &Path, config: &BuffAssistantConfig) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| format!("创建 Buff 助手目录失败：{error}"))?;
    let mut json = serde_json::to_string_pretty(config)
        .map_err(|error| format!("序列化 Buff 助手配置失败：{error}"))?;
    json.push('\n');
    fs::write(directory.join(CONFIG_FILE), json)
        .map_err(|error| format!("保存 Buff 助手配置失败：{error}"))
}

pub fn load_sound_templates(directory: &Path) -> (Vec<SoundTemplate>, Vec<String>) {
    let mut templates = Vec::new();
    let mut notices = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        notices.push("没有找到内置提示音模板资源".into());
        return (templates, notices);
    };
    for entry in entries.flatten().filter(|entry| entry.path().is_dir()) {
        let template_directory = entry.path();
        let result = fs::read_to_string(template_directory.join("manifest.json"))
            .map_err(|error| error.to_string())
            .and_then(|contents| {
                serde_json::from_str::<SoundTemplateManifest>(&contents)
                    .map_err(|error| error.to_string())
            })
            .and_then(|manifest| validate_sound_template(template_directory, manifest));
        match result {
            Ok(template) => templates.push(template),
            Err(error) => notices.push(format!("忽略损坏的提示音模板：{error}")),
        }
    }
    templates.sort_by(|left, right| left.summary.name.cmp(&right.summary.name));
    (templates, notices)
}

pub fn template_sound_path(
    templates: &[SoundTemplate],
    template_id: &str,
    cue: BuffSoundCue,
) -> Option<PathBuf> {
    let template = templates
        .iter()
        .find(|template| template.summary.id == template_id)?;
    let file_name = match cue {
        BuffSoundCue::Triggered => &template.files.triggered,
        BuffSoundCue::PrewarnThree => &template.files.prewarn_three,
        BuffSoundCue::PrewarnTwo => &template.files.prewarn_two,
        BuffSoundCue::PrewarnOne => &template.files.prewarn_one,
    };
    Some(template.directory.join(file_name))
}

pub fn import_sound_asset(
    directory: &Path,
    source: &Path,
    asset_id: &str,
) -> Result<BuffCustomSoundAsset, String> {
    validate_sound_asset_candidate(source)?;
    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "无法读取 WAV 文件名".to_string())?
        .to_string();
    let target_directory = directory.join(SOUND_ASSETS_DIRECTORY);
    fs::create_dir_all(&target_directory)
        .map_err(|error| format!("创建自定义声音目录失败：{error}"))?;
    let target = target_directory.join(format!("{}.wav", safe_asset_id(asset_id)?));
    fs::copy(source, target).map_err(|error| format!("保存自定义 WAV 失败：{error}"))?;
    Ok(BuffCustomSoundAsset {
        asset_id: asset_id.to_string(),
        file_name,
    })
}

pub fn validate_sound_asset_candidate(source: &Path) -> Result<(), String> {
    let metadata = fs::metadata(source).map_err(|error| format!("读取 WAV 文件失败：{error}"))?;
    if !metadata.is_file() {
        return Err("请选择 WAV 文件".into());
    }
    if metadata.len() > MAX_SOUND_FILE_BYTES {
        return Err("WAV 文件不能超过 10 MiB".into());
    }
    if source
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("wav"))
    {
        return Err("仅支持 WAV 文件".into());
    }
    Ok(())
}

pub fn custom_sound_path(directory: &Path, asset_id: &str) -> Result<PathBuf, String> {
    Ok(directory
        .join(SOUND_ASSETS_DIRECTORY)
        .join(format!("{}.wav", safe_asset_id(asset_id)?)))
}

pub fn validate_sound_sources(
    directory: &Path,
    templates: &[SoundTemplate],
    config: &BuffAssistantConfig,
) -> Result<(), String> {
    for source in sound_sources(config) {
        match source {
            BuffSoundSource::Sine => {}
            BuffSoundSource::Template { template_id } => {
                if !templates
                    .iter()
                    .any(|template| template.summary.id == *template_id)
                {
                    return Err(format!("提示音模板不存在：{template_id}"));
                }
            }
            BuffSoundSource::Custom { asset_id, .. } => {
                if !custom_sound_path(directory, asset_id)?.is_file() {
                    return Err("自定义提示音文件不存在，请重新上传".into());
                }
            }
        }
    }
    Ok(())
}

fn validate_sound_template(
    directory: PathBuf,
    manifest: SoundTemplateManifest,
) -> Result<SoundTemplate, String> {
    if manifest.id.is_empty()
        || !manifest
            .id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("模板 ID 无效".into());
    }
    if manifest.name.trim().is_empty() {
        return Err(format!("模板 {} 没有显示名称", manifest.id));
    }
    for file_name in [
        &manifest.files.triggered,
        &manifest.files.prewarn_three,
        &manifest.files.prewarn_two,
        &manifest.files.prewarn_one,
    ] {
        let path = Path::new(file_name);
        if path.file_name() != Some(path.as_os_str())
            || path
                .extension()
                .and_then(|value| value.to_str())
                .is_none_or(|value| !value.eq_ignore_ascii_case("wav"))
            || !directory.join(path).is_file()
        {
            return Err(format!("模板 {} 的 WAV 文件不完整", manifest.id));
        }
        audio::validate_wav_file(&directory.join(path))
            .map_err(|error| format!("模板 {} 的 WAV 无效：{error}", manifest.id))?;
    }
    Ok(SoundTemplate {
        summary: BuffSoundTemplateSummary {
            id: manifest.id,
            name: manifest.name,
        },
        directory,
        files: manifest.files,
    })
}

pub fn cleanup_unused_sound_assets(directory: &Path, config: &BuffAssistantConfig) {
    let assets_directory = directory.join(SOUND_ASSETS_DIRECTORY);
    let Ok(entries) = fs::read_dir(&assets_directory) else {
        return;
    };
    let used = sound_sources(config)
        .filter_map(|source| match source {
            BuffSoundSource::Custom { asset_id, .. } => Some(format!("{asset_id}.wav")),
            _ => None,
        })
        .collect::<std::collections::HashSet<_>>();
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !used.contains(&file_name) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn sound_sources(config: &BuffAssistantConfig) -> impl Iterator<Item = &BuffSoundSource> {
    let sound = &config.settings.sound;
    [
        &sound.trigger_source,
        &sound.prewarn_three_source,
        &sound.prewarn_two_source,
        &sound.prewarn_one_source,
    ]
    .into_iter()
}

fn safe_asset_id(asset_id: &str) -> Result<&str, String> {
    if !asset_id.is_empty()
        && asset_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        Ok(asset_id)
    } else {
        Err("自定义提示音标识无效".into())
    }
}

pub fn save_template(
    directory: &Path,
    id: &str,
    image: &DynamicImage,
    mask: &GrayImage,
) -> Result<BuffTemplateSummary, String> {
    let target = template_directory(directory, id);
    fs::create_dir_all(&target).map_err(|error| format!("创建模板目录失败：{error}"))?;
    image
        .save(target.join("template.png"))
        .map_err(|error| format!("保存模板图片失败：{error}"))?;
    mask.save(target.join("mask.png"))
        .map_err(|error| format!("保存模板遮罩失败：{error}"))?;
    Ok(BuffTemplateSummary {
        id: id.to_string(),
        width: image.width(),
        height: image.height(),
    })
}

pub fn load_template(
    directory: &Path,
    summary: &BuffTemplateSummary,
) -> Result<TemplateData, String> {
    let target = template_directory(directory, &summary.id);
    let image = image::open(target.join("template.png"))
        .map_err(|error| format!("读取模板图片失败：{error}"))?
        .into_luma8();
    let mask = image::open(target.join("mask.png"))
        .map_err(|error| format!("读取模板遮罩失败：{error}"))?
        .into_luma8();
    TemplateData::new(image, mask)
}

pub fn delete_template(directory: &Path, summary: &BuffTemplateSummary) -> Result<(), String> {
    let target = template_directory(directory, &summary.id);
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|error| format!("删除模板失败：{error}"))?;
    }
    Ok(())
}

fn template_directory(directory: &Path, id: &str) -> PathBuf {
    let safe_id = id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect::<String>();
    directory.join("templates").join(safe_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_sound_template_is_complete_and_decodable() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("buff-sounds");
        let (templates, notices) = load_sound_templates(&directory);
        assert!(notices.is_empty(), "{notices:?}");
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].summary.id, "template-1");
        assert_eq!(templates[0].summary.name, "模板一");
        for cue in [
            BuffSoundCue::Triggered,
            BuffSoundCue::PrewarnThree,
            BuffSoundCue::PrewarnTwo,
            BuffSoundCue::PrewarnOne,
        ] {
            let path = template_sound_path(&templates, "template-1", cue).unwrap();
            audio::validate_wav_file(&path).unwrap();
        }
    }

    #[test]
    fn unsafe_custom_asset_ids_are_rejected() {
        let directory = std::env::temp_dir().join("macro-flow-sound-path-test");
        assert!(custom_sound_path(&directory, "../outside").is_err());
        assert!(custom_sound_path(&directory, "safe-id-123").is_ok());
    }

    #[test]
    fn oversized_custom_sound_is_rejected_before_decoding() {
        let path = std::env::temp_dir().join(format!(
            "macro-flow-oversized-sound-{}.wav",
            std::process::id()
        ));
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_SOUND_FILE_BYTES + 1).unwrap();
        drop(file);
        let error = validate_sound_asset_candidate(&path).unwrap_err();
        assert!(error.contains("10 MiB"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn custom_sound_is_copied_and_removed_when_unreferenced() {
        let directory =
            std::env::temp_dir().join(format!("macro-flow-sound-storage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("buff-sounds")
            .join("template-1")
            .join("triggered.wav");
        let asset = import_sound_asset(&directory, &source, "triggered-test").unwrap();
        let path = custom_sound_path(&directory, &asset.asset_id).unwrap();
        assert!(path.is_file());

        let mut config = BuffAssistantConfig::default();
        config.settings.sound.trigger_source = BuffSoundSource::Custom {
            asset_id: asset.asset_id.clone(),
            file_name: asset.file_name,
        };
        cleanup_unused_sound_assets(&directory, &config);
        assert!(path.is_file());

        config.settings.sound.trigger_source = BuffSoundSource::Sine;
        cleanup_unused_sound_assets(&directory, &config);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(directory);
    }
}
