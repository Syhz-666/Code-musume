use base64::{engine::general_purpose::STANDARD, Engine as _};
use code_musume_contracts::*;
use code_musume_hook::runtime::{atomic_write, config_dir};
use image::{GenericImageView, ImageReader};
use std::{
    collections::BTreeMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn root() -> PathBuf {
    config_dir().join("characters")
}
fn clips() -> BTreeMap<String, Clip> {
    [
        ("idle", 0, 6),
        ("thinking", 7, 6),
        ("working", 7, 6),
        ("waiting-permission", 6, 6),
        ("done", 8, 6),
        ("notification", 3, 4),
        ("error", 5, 8),
        ("sleeping", 0, 1),
        ("touch", 4, 5),
        ("drag-right", 1, 8),
        ("drag-left", 2, 8),
    ]
    .into_iter()
    .map(|(s, row, frames)| {
        (
            s.into(),
            Clip {
                row,
                frames,
                fps: if s == "idle" { 6.0 } else { 10.0 },
                looping: !matches!(s, "touch" | "done" | "notification"),
            },
        )
    })
    .collect()
}
pub fn builtin() -> CharacterManifest {
    CharacterManifest {
        schema_version: 1,
        id: "builtin".into(),
        name: "代码娘 · 团子".into(),
        default_form: "default".into(),
        agent_forms: BTreeMap::from([
            ("claude-code".into(), "claude".into()),
            ("codex".into(), "codex".into()),
        ]),
        forms: ["default", "claude", "codex"]
            .into_iter()
            .map(|id| CharacterForm {
                id: id.into(),
                image: String::new(),
                cell_width: 192,
                cell_height: 208,
                columns: 8,
                clips: clips(),
                look_rows: false,
            })
            .collect(),
        license: Some("MIT (procedural sample artwork)".into()),
    }
}
fn image_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if fs::metadata(path).map_err(|_| "图片不存在")?.len() > 20 * 1024 * 1024 {
        return Err("图片不能超过 20MiB".into());
    }
    fs::read(path).map_err(|e| e.to_string())
}
fn decode(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::WebP)
    ) {
        return Err("仅接受 PNG / WebP".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|_| "图片损坏或解码资源超限".into())
}
fn validate(m: &CharacterManifest, dir: &Path) -> Result<(), String> {
    if m.schema_version != 1
        || !valid_id(&m.id)
        || m.forms.is_empty()
        || m.forms.len() > 16
        || !m.forms.iter().any(|f| f.id == m.default_form)
    {
        return Err("角色元信息无效".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for f in &m.forms {
        if !valid_id(&f.id)
            || !ids.insert(&f.id)
            || f.cell_width == 0
            || f.cell_height == 0
            || f.cell_width > 1024
            || f.cell_height > 1024
            || f.columns == 0
            || f.columns > 64
        {
            return Err("形态布局无效".into());
        }
        if Path::new(&f.image).components().count() != 1
            || !matches!(
                Path::new(&f.image).extension().and_then(|s| s.to_str()),
                Some("png" | "webp")
            )
        {
            return Err("图片必须是包内 PNG / WebP 文件".into());
        }
        let path = dir
            .join(&f.image)
            .canonicalize()
            .map_err(|_| "角色图片不存在")?;
        let canonical = dir.canonicalize().map_err(|e| e.to_string())?;
        if !path.starts_with(&canonical) {
            return Err("角色图片路径越界".into());
        }
        let bytes = image_bytes(&path)?;
        let format = image::guess_format(&bytes).map_err(|_| "图片格式无效")?;
        let expected = if f.image.ends_with(".png") {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::WebP
        };
        if format != expected {
            return Err("图片扩展名与内容不一致".into());
        }
        let img = decode(&bytes)?;
        let (w, h) = img.dimensions();
        if w != f.cell_width * f.columns
            || h % f.cell_height != 0
            || f.clips.is_empty()
            || !f.clips.contains_key("idle")
        {
            return Err("图集尺寸或动作无效".into());
        }
        for c in f.clips.values() {
            if c.row >= h / f.cell_height
                || c.frames == 0
                || c.frames > f.columns
                || !c.fps.is_finite()
                || !(1.0..=60.0).contains(&c.fps)
            {
                return Err("动画行、帧数或帧率无效".into());
            }
        }
    }
    if m.agent_forms
        .values()
        .any(|id| !m.forms.iter().any(|f| &f.id == id))
    {
        return Err("agent 映射引用了不存在的形态".into());
    }
    Ok(())
}
pub fn load(id: &str) -> Result<CharacterManifest, String> {
    if id == "builtin" {
        return Ok(builtin());
    }
    if !valid_id(id) {
        return Err("角色 ID 无效".into());
    }
    let dir = root().join(id);
    let b = fs::read(dir.join("musume.json")).map_err(|_| "找不到角色包")?;
    if b.len() > 256 * 1024 {
        return Err("角色配置过大".into());
    }
    let m: CharacterManifest = serde_json::from_slice(&b).map_err(|_| "角色配置损坏")?;
    if m.id != id {
        return Err("角色目录与 ID 不一致".into());
    }
    validate(&m, &dir)?;
    Ok(m)
}
pub fn list() -> Vec<CharacterManifest> {
    let mut out = vec![builtin()];
    if let Ok(entries) = fs::read_dir(root()) {
        for e in entries.flatten() {
            if let Some(id) = e.file_name().to_str() {
                if let Ok(m) = load(id) {
                    out.push(m);
                }
            }
        }
    }
    out
}
pub fn asset(id: &str, form: &str) -> Result<String, String> {
    let m = load(id)?;
    let f = m.forms.iter().find(|f| f.id == form).ok_or("找不到形态")?;
    if id == "builtin" {
        return Ok(String::new());
    }
    let b = image_bytes(&root().join(id).join(&f.image))?;
    let mime = if f.image.ends_with(".png") {
        "image/png"
    } else {
        "image/webp"
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(b)))
}
pub fn import(source: &Path) -> Result<CharacterManifest, String> {
    let dir = if source.is_dir() {
        source.to_path_buf()
    } else {
        source.parent().ok_or("来源路径无效")?.to_path_buf()
    };
    if source.is_dir() && dir.join("musume.json").exists() {
        let b = fs::read(dir.join("musume.json")).map_err(|e| e.to_string())?;
        if b.len() > 256 * 1024 {
            return Err("角色配置过大".into());
        }
        let mut m: CharacterManifest = serde_json::from_slice(&b).map_err(|_| "角色配置损坏")?;
        validate(&m, &dir)?;
        m.id = format!("import-{}", uuid::Uuid::new_v4().simple());
        let target = root().join(&m.id);
        fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        for f in &m.forms {
            fs::copy(dir.join(&f.image), target.join(&f.image)).map_err(|e| e.to_string())?;
        }
        copy_metadata(&dir, &target)?;
        atomic_write(&target.join("musume.original.json"), &b)?;
        atomic_write(
            &target.join("musume.json"),
            &serde_json::to_vec_pretty(&m).unwrap(),
        )?;
        return Ok(m);
    }
    let image = if source.is_file() {
        source.to_path_buf()
    } else {
        [dir.join("spritesheet.webp"), dir.join("spritesheet.png")]
            .into_iter()
            .find(|p| p.is_file())
            .ok_or("目录中没有 spritesheet.png / webp")?
    };
    let b = image_bytes(&image)?;
    let img = decode(&b)?;
    validate_codex_sheet(&img)?;
    let h = img.height();
    let id = format!("import-{}", uuid::Uuid::new_v4().simple());
    let target = root().join(&id);
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    let ext = if image::guess_format(&b).ok() == Some(image::ImageFormat::Png) {
        "png"
    } else {
        "webp"
    };
    let name = format!("spritesheet.{ext}");
    atomic_write(&target.join(&name), &b)?;
    let mut label = "导入角色".to_string();
    let license = copy_metadata(&dir, &target)?;
    if let Ok(data) = fs::read(target.join("pet.original.json")) {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&data) {
            label = v["displayName"]
                .as_str()
                .or_else(|| v["name"].as_str())
                .unwrap_or(&label)
                .chars()
                .take(80)
                .collect();
        }
    }
    let m = CharacterManifest {
        schema_version: 1,
        id,
        name: label,
        default_form: "default".into(),
        agent_forms: BTreeMap::new(),
        forms: vec![CharacterForm {
            id: "default".into(),
            image: name,
            cell_width: 192,
            cell_height: 208,
            columns: 8,
            clips: clips(),
            look_rows: h == 2288,
        }],
        license,
    };
    atomic_write(
        &target.join("musume.json"),
        &serde_json::to_vec_pretty(&m).unwrap(),
    )?;
    Ok(m)
}

fn validate_codex_sheet(img: &image::DynamicImage) -> Result<(), String> {
    let (w, h) = img.dimensions();
    if w != 1536 || !matches!(h, 1872 | 2288) {
        return Err("Codex 图集必须为 1536×1872（v1）或 1536×2288（v2）".into());
    }
    let rgba = img.to_rgba8();
    let counts = [6, 8, 8, 4, 5, 8, 6, 6, 6, 8, 8];
    for row in 0..h / 208 {
        for col in 0..8 {
            let mut visible = false;
            for y in row * 208..(row + 1) * 208 {
                for x in col * 192..(col + 1) * 192 {
                    visible |= rgba.get_pixel(x, y).0[3] > 0;
                }
            }
            if (col < counts[row as usize]) != visible {
                return Err(format!(
                    "图集第 {} 行第 {} 格为空或填充了应透明的格",
                    row + 1,
                    col + 1
                ));
            }
        }
    }
    Ok(())
}

fn copy_metadata(dir: &Path, target: &Path) -> Result<Option<String>, String> {
    let canonical = dir.canonicalize().map_err(|e| e.to_string())?;
    let mut license = None;
    for name in [
        "pet.json",
        "LICENSE",
        "LICENSE.txt",
        "LICENSE.md",
        "ATTRIBUTION.md",
    ] {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        if !path.starts_with(&canonical) {
            return Err("授权或元数据路径越界".into());
        }
        if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 64 * 1024 {
            return Err("授权或元数据超过 64KiB".into());
        }
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        atomic_write(
            &target.join(if name == "pet.json" {
                "pet.original.json"
            } else {
                name
            }),
            &bytes,
        )?;
        if name.starts_with("LICENSE") {
            license = Some(format!("原包授权见 {name}"));
        }
    }
    Ok(license)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sheet(rows: u32) -> image::DynamicImage {
        let mut pixels = image::RgbaImage::new(1536, rows * 208);
        for row in 0..rows {
            for col in 0..[6, 8, 8, 4, 5, 8, 6, 6, 6, 8, 8][row as usize] {
                pixels.put_pixel(
                    col * 192 + 96,
                    row * 208 + 104,
                    image::Rgba([100, 120, 150, 255]),
                );
            }
        }
        image::DynamicImage::ImageRgba8(pixels)
    }
    #[test]
    fn codex_v1_and_v2_structural_layouts_are_supported() {
        assert!(validate_codex_sheet(&sheet(9)).is_ok());
        assert!(validate_codex_sheet(&sheet(11)).is_ok());
    }
    #[test]
    fn unknown_layout_blank_frames_and_used_padding_are_rejected() {
        assert!(validate_codex_sheet(&sheet(10)).is_err());
        assert!(validate_codex_sheet(&image::DynamicImage::new_rgba8(1536, 1872)).is_err());
        let mut invalid = sheet(9).into_rgba8();
        invalid.put_pixel(7 * 192, 0, image::Rgba([0, 0, 0, 255]));
        assert!(validate_codex_sheet(&image::DynamicImage::ImageRgba8(invalid)).is_err());
    }
    #[test]
    fn corrupt_images_and_traversal_ids_are_rejected() {
        assert!(decode(b"not an image").is_err());
        for id in ["../other", "C:/other", "", "one/two", "one\\two"] {
            assert!(!valid_id(id));
        }
    }
}
