use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Describes an event organization template so PhotoFinish works cleanly for
/// gymnastics meets out of the box without hardcoding gymnastics-only concepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPreset {
    pub id: String,
    pub label: String,
    pub description: String,
    pub subject_label: String,
    /// Ordered folder levels above the image filename.
    /// The final level typically represents the individual subject folder.
    pub folder_levels: Vec<String>,
}

/// Optional scaffolding parameters when creating an event's initial folder tree.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScaffoldConfig {
    /// For gymnastics-style events: number of top-level areas (e.g. Gym A, Gym B).
    pub num_areas: Option<u32>,
    /// Label prefix for top-level areas (e.g. "Gym", "Stage", "Floor").
    pub area_prefix: Option<String>,
    /// Number of sessions per area (e.g. Session 1A, Session 2A).
    pub sessions_per_area: Option<u32>,
    /// Label prefix for sessions (e.g. "Session").
    pub session_prefix: Option<String>,
}

pub fn builtin_presets() -> Vec<EventPreset> {
    vec![
        EventPreset {
            id: "gymnastics".to_string(),
            label: "Gymnastics Meet".to_string(),
            description: "Gym -> Session -> Group -> Apparatus -> Competitor".to_string(),
            subject_label: "Competitor".to_string(),
            folder_levels: vec![
                "Gym".to_string(),
                "Session".to_string(),
                "Group".to_string(),
                "Apparatus".to_string(),
                "Subject".to_string(),
            ],
        },
        EventPreset {
            id: "dance".to_string(),
            label: "Dance / Stage Competition".to_string(),
            description: "Stage -> Session -> Routine -> Dancer".to_string(),
            subject_label: "Dancer".to_string(),
            folder_levels: vec![
                "Stage".to_string(),
                "Session".to_string(),
                "Routine".to_string(),
                "Subject".to_string(),
            ],
        },
        EventPreset {
            id: "simple".to_string(),
            label: "Simple / Club / Picture Day".to_string(),
            description: "Group -> Participant".to_string(),
            subject_label: "Participant".to_string(),
            folder_levels: vec!["Group".to_string(), "Subject".to_string()],
        },
        EventPreset {
            id: "custom".to_string(),
            label: "Custom Hierarchy".to_string(),
            description: "Define your own folder levels and subject label".to_string(),
            subject_label: "Participant".to_string(),
            folder_levels: vec!["Group".to_string(), "Subject".to_string()],
        },
    ]
}

pub fn find_preset(id: &str) -> Option<EventPreset> {
    builtin_presets().into_iter().find(|p| p.id == id)
}

/// Extracts flexible key-value location tags from a photo's relative path
/// using the event's configured `folder_levels`.
///
/// Unlike a rigid path parser, this NEVER fails when a photographer uses fewer
/// or deeper folders than expected:
/// - Matches directory segments left-to-right against `folder_levels`
/// - Records `leaf_folder` (the immediate parent directory) regardless of depth
/// - Stores any extra segments under `extra_path` so no context is ever dropped
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLocation {
    pub folder_path: String,
    pub leaf_folder: Option<String>,
    pub filename: String,
    pub tags: BTreeMap<String, String>,
}

pub fn parse_relative_location(relative_path: &str, folder_levels: &[String]) -> ParsedLocation {
    let normalized = relative_path.replace('\\', "/");
    let parts: Vec<&str> = normalized
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        return ParsedLocation {
            folder_path: String::new(),
            leaf_folder: None,
            filename: String::new(),
            tags: BTreeMap::new(),
        };
    }

    let filename = parts[parts.len() - 1].to_string();
    let dir_segments = &parts[..parts.len() - 1];
    let folder_path = dir_segments.join("/");
    let leaf_folder = dir_segments.last().map(|s| (*s).to_string());

    let mut tags = BTreeMap::new();
    for (idx, seg) in dir_segments.iter().enumerate() {
        if let Some(level_name) = folder_levels.get(idx) {
            tags.insert(level_name.clone(), (*seg).to_string());
        } else {
            let extra_key = format!("level_{}", idx + 1);
            tags.insert(extra_key, (*seg).to_string());
        }
    }

    ParsedLocation {
        folder_path,
        leaf_folder,
        filename,
        tags,
    }
}

/// Creates initial area/session folders inside `event_root` based on `ScaffoldConfig`.
/// Example: num_areas=2, area_prefix="Gym", sessions_per_area=3, session_prefix="Session"
/// creates `Gym A/Session 1A`, `Gym A/Session 2A`, ..., `Gym B/Session 3B`.
pub fn scaffold_folders(event_root: &Path, config: &ScaffoldConfig) -> Result<Vec<PathBuf>> {
    let num_areas = config.num_areas.unwrap_or(0);
    let sessions_per_area = config.sessions_per_area.unwrap_or(0);

    if num_areas == 0 {
        return Ok(Vec::new());
    }

    let area_prefix = config.area_prefix.as_deref().unwrap_or("Gym").trim();
    let session_prefix = config.session_prefix.as_deref().unwrap_or("Session").trim();

    let mut created = Vec::new();
    for area_idx in 0..num_areas.min(26) {
        let letter = (b'A' + area_idx as u8) as char;
        let area_dir = format!("{} {}", area_prefix, letter);

        if sessions_per_area == 0 {
            let path = event_root.join(&area_dir);
            fs::create_dir_all(&path).map_err(|e| Error::Io(format!("{}: {}", path.display(), e)))?;
            created.push(path);
        } else {
            for session_num in 1..=sessions_per_area {
                let session_dir = format!("{} {}{}", session_prefix, session_num, letter);
                let path = event_root.join(&area_dir).join(&session_dir);
                fs::create_dir_all(&path)
                    .map_err(|e| Error::Io(format!("{}: {}", path.display(), e)))?;
                created.push(path);
            }
        }
    }

    Ok(created)
}

/// Extracts a leading numeric subject/competitor number from a folder name
/// such as `"1059 Iza Z"` or `"1022"` -> `Some("1059")`.
pub fn extract_subject_number(folder_name: &str) -> Option<String> {
    let trimmed = folder_name.trim();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    // Must be either the whole folder name or followed by whitespace/separator
    let rest = &trimmed[digits.len()..];
    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('-') || rest.starts_with('_') {
        Some(digits)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_gymnastics_and_non_standard_paths_without_failing() {
        let gym_levels = find_preset("gymnastics").unwrap().folder_levels;
        let parsed = parse_relative_location(
            "Gym A/Session 1A/Group 2B/Beam/1059 Iza Z/IMG_001.jpg",
            &gym_levels,
        );
        assert_eq!(parsed.filename, "IMG_001.jpg");
        assert_eq!(parsed.leaf_folder.as_deref(), Some("1059 Iza Z"));
        assert_eq!(parsed.tags.get("Gym").map(String::as_str), Some("Gym A"));
        assert_eq!(parsed.tags.get("Apparatus").map(String::as_str), Some("Beam"));
        assert_eq!(parsed.tags.get("Subject").map(String::as_str), Some("1059 Iza Z"));

        // Shorter path (e.g. single-gym meet skipping Gym folder) still parses gracefully
        let short = parse_relative_location("Session 1/Beam/IMG_002.jpg", &gym_levels);
        assert_eq!(short.filename, "IMG_002.jpg");
        assert_eq!(short.leaf_folder.as_deref(), Some("Beam"));
        assert_eq!(short.tags.len(), 2);
    }

    #[test]
    fn scaffolds_configurable_area_and_session_folders() {
        let dir = tempdir().unwrap();
        let created = scaffold_folders(
            dir.path(),
            &ScaffoldConfig {
                num_areas: Some(2),
                area_prefix: Some("Stage".to_string()),
                sessions_per_area: Some(2),
                session_prefix: Some("Block".to_string()),
            },
        )
        .unwrap();

        assert_eq!(created.len(), 4);
        assert!(dir.path().join("Stage A/Block 1A").is_dir());
        assert!(dir.path().join("Stage B/Block 2B").is_dir());
    }

    #[test]
    fn extracts_leading_subject_numbers() {
        assert_eq!(extract_subject_number("1059 Iza Z").as_deref(), Some("1059"));
        assert_eq!(extract_subject_number("1022").as_deref(), Some("1022"));
        assert_eq!(extract_subject_number("Gymnast 05"), None);
        assert_eq!(extract_subject_number("105MSDCF"), None);
    }
}
