use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{DynamicImage, GrayImage};
use tauri::{AppHandle, Manager};

use super::{
    detector::TemplateData,
    model::{BuffAssistantConfig, BuffTemplateSummary},
};

const CONFIG_FILE: &str = "config-v1.json";

pub fn storage_directory(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("buff-assistant"))
        .map_err(|error| format!("无法确定 Buff 助手配置目录：{error}"))
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
        notices.push("Buff 图标模板文件不存在，请重新采集".into());
        config.template = None;
    }
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
