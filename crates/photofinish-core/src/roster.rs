use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterEntryInput {
    pub number: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub group_label: Option<String>,
    pub team_name: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRosterFile {
    pub file_path: String,
    pub filename: String,
    pub default_group_label: Option<String>,
    pub entries: Vec<RosterEntryInput>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRosterResult {
    pub imported_new: usize,
    pub updated: usize,
    pub auto_matched_photos: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderItem {
    pub folder_path: String,
    pub parent_path: String,
    pub leaf_folder: String,
    pub photo_count: i64,
    pub unassigned_count: i64,
    pub assigned_subject_id: Option<String>,
    pub assigned_subject_name: Option<String>,
    pub assigned_subject_number: Option<String>,
    pub is_mixed: bool,
    pub sample_photo_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderLocationSummary {
    pub parent_path: String,
    pub folder_count: usize,
    pub photo_count: i64,
    pub unassigned_folder_count: usize,
    pub unassigned_photo_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFoldersResponse {
    pub locations: Vec<FolderLocationSummary>,
    pub folders: Vec<FolderItem>,
    pub total_unassigned_photos: i64,
    pub total_unassigned_folders: usize,
    pub detected_session_group: Option<String>,
    pub detected_group_prefix: Option<String>,
    pub same_group_subject_ids: Vec<String>,
    pub subject_sample_photos: BTreeMap<String, String>,
    /// Apparatus / section name inferred from the active folder location (e.g. "Beam", "Floor").
    pub section_label: Option<String>,
    /// Subjects already matched on the SAME apparatus/section in this session
    /// (subject_id -> short group/location label like "Group 1A").
    pub section_matched_subjects: BTreeMap<String, String>,
}

/// Summary of an ingested memory card (or folder batch) for the Card Naming Queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestBatchSummary {
    pub id: String,
    pub sequence_num: i64,
    pub reader_id: Option<String>,
    pub reader_name: Option<String>,
    pub volume_name: Option<String>,
    pub photographer: Option<String>,
    pub destination_rel_path: String,
    pub total_photos: i64,
    pub ready_photos: i64,
    pub pending_photos: i64,
    pub total_folders: i64,
    pub unassigned_folders: i64,
    pub unassigned_photos: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderReassignmentInput {
    pub folder_path: String,
    pub target_subject_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderChangeResultItem {
    pub old_folder_path: String,
    pub new_folder_path: String,
    pub target_subject_id: Option<String>,
    pub target_subject_name: Option<String>,
    pub photos_updated: usize,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReassignResult {
    pub folders_updated: usize,
    pub photos_updated: usize,
    pub renamed_on_disk: bool,
    pub changes: Vec<FolderChangeResultItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoveFolderMode {
    Trash,
    DeleteDisk,
    DbOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveFolderResult {
    pub folder_path: String,
    pub photos_removed: usize,
    pub mode: RemoveFolderMode,
    pub trashed_to: Option<String>,
}

/// Parses a `.txt` or `.csv` roster file into structured `RosterEntryInput` rows.
pub fn parse_roster_file(path: &Path) -> Result<ParsedRosterFile> {
    let content = fs::read_to_string(path)
        .map_err(|e| Error::Io(format!("Failed to read {}: {}", path.display(), e)))?;
    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "roster.txt".to_string());
    let default_group_label = path
        .file_stem()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty());

    let is_csv = path
        .extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("csv"));

    let (entries, warnings) = if is_csv {
        parse_csv_content(&content, default_group_label.as_deref())
    } else {
        parse_txt_content(&content, default_group_label.as_deref())
    };

    Ok(ParsedRosterFile {
        file_path: path.to_string_lossy().to_string(),
        filename,
        default_group_label,
        entries,
        warnings,
    })
}

pub fn parse_txt_content(
    content: &str,
    default_group_label: Option<&str>,
) -> (Vec<RosterEntryInput>, Vec<String>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let first_tok = parts.next().unwrap_or("").trim();
        let rest = parts.next().unwrap_or("").trim();

        let looks_like_number = !first_tok.is_empty()
            && first_tok.chars().all(|c| c.is_ascii_digit())
            && !rest.is_empty();

        let (number, name_src) = if looks_like_number {
            (Some(first_tok.to_string()), rest)
        } else {
            (None, line)
        };
        let (first_name, last_name) = split_name(name_src);
        if first_name.is_empty() {
            warnings.push(format!("Line {}: could not parse '{}'", idx + 1, line));
            continue;
        }

        entries.push(RosterEntryInput {
            number,
            first_name,
            last_name,
            group_label: default_group_label.map(str::to_string),
            team_name: None,
            external_id: None,
        });
    }

    (entries, warnings)
}

pub fn parse_csv_content(
    content: &str,
    default_group_label: Option<&str>,
) -> (Vec<RosterEntryInput>, Vec<String>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let mut lines = content
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim().trim_start_matches('\u{feff}')))
        .filter(|(_, l)| !l.is_empty());

    let Some((_, header_line)) = lines.next() else {
        return (entries, warnings);
    };

    let headers: Vec<String> = split_csv_row(header_line)
        .into_iter()
        .map(|h| h.trim().to_lowercase().replace([' ', '-'], "_"))
        .collect();

    let col_num = find_col(&headers, &["number", "competitor_number", "bib", "athlete_number", "id"]);
    let col_first = find_col(&headers, &["first_name", "first", "fname"]);
    let col_last = find_col(&headers, &["last_name", "last", "lname"]);
    let col_full = find_col(&headers, &["name", "full_name", "display_name", "competitor", "athlete"]);
    let col_group = find_col(&headers, &["group_label", "group", "session", "division", "level", "heat"]);
    let col_team = find_col(&headers, &["team_name", "team", "club", "gym", "school"]);
    let col_ext = find_col(&headers, &["external_id", "usag", "usag_id", "member_id"]);

    if col_first.is_none() && col_full.is_none() {
        warnings.push("CSV header must include a 'first_name' or 'name' column".to_string());
        return (entries, warnings);
    }

    for (line_num, line) in lines {
        let cols = split_csv_row(line);
        let get = |idx: Option<usize>| -> Option<String> {
            idx.and_then(|i| cols.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };

        let (first_name, last_name) = if let Some(first) = get(col_first) {
            (first, get(col_last))
        } else if let Some(full) = get(col_full) {
            split_name(&full)
        } else {
            warnings.push(format!("Line {}: missing name", line_num));
            continue;
        };

        if first_name.is_empty() {
            warnings.push(format!("Line {}: empty first name", line_num));
            continue;
        }

        entries.push(RosterEntryInput {
            number: get(col_num),
            first_name,
            last_name,
            group_label: get(col_group).or_else(|| default_group_label.map(str::to_string)),
            team_name: get(col_team),
            external_id: get(col_ext),
        });
    }

    (entries, warnings)
}

fn find_col(headers: &[String], candidates: &[&str]) -> Option<usize> {
    candidates
        .iter()
        .find_map(|c| headers.iter().position(|h| h == c))
}

fn split_name(name: &str) -> (String, Option<String>) {
    let trimmed = name.trim();
    match trimmed.split_once(char::is_whitespace) {
        Some((first, rest)) => {
            let last = rest.trim();
            (
                first.trim().to_string(),
                (!last.is_empty()).then(|| last.to_string()),
            )
        }
        None => (trimmed.to_string(), None),
    }
}

fn split_csv_row(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' if !in_quotes && current.is_empty() => {
                in_quotes = true;
            }
            ',' if !in_quotes => {
                fields.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// Natural sort key helper so `"Session 2A"` sorts before `"Session 10A"`.
pub fn natural_sort_key(s: &str) -> Vec<(u8, u64, String)> {
    let mut key = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            key.push((0, num.parse::<u64>().unwrap_or(0), String::new()));
        } else {
            let mut text = String::new();
            while let Some(&ch) = chars.peek() {
                if !ch.is_ascii_digit() {
                    text.push(ch.to_ascii_lowercase());
                    chars.next();
                } else {
                    break;
                }
            }
            key.push((1, 0, text));
        }
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_crlf_txt_roster() {
        let txt = "143 Avery W\r\n169 Callie W\r\n\r\n105 Julia B\r\n";
        let (entries, warnings) = parse_txt_content(txt, Some("Session 1A"));
        assert!(warnings.is_empty());
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].number.as_deref(), Some("143"));
        assert_eq!(entries[0].first_name, "Avery");
        assert_eq!(entries[0].last_name.as_deref(), Some("W"));
        assert_eq!(entries[0].group_label.as_deref(), Some("Session 1A"));
    }

    #[test]
    fn parses_csv_roster_with_quotes_and_team() {
        let csv = "competitor_number,first_name,last_name,session,team\n1022,Kevin,Smith,1A,\"Bay Area, CA\"\n1023,Sarah,Johnson,2B,Elite\n";
        let (entries, warnings) = parse_csv_content(csv, None);
        assert!(warnings.is_empty());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].number.as_deref(), Some("1022"));
        assert_eq!(entries[0].team_name.as_deref(), Some("Bay Area, CA"));
        assert_eq!(entries[1].group_label.as_deref(), Some("2B"));
    }

    #[test]
    fn natural_sorts_sessions_properly() {
        let mut sessions = vec!["Session 10A", "Session 2A", "Session 1A", "Session 11A"];
        sessions.sort_by_key(|s| natural_sort_key(s));
        assert_eq!(sessions, vec!["Session 1A", "Session 2A", "Session 10A", "Session 11A"]);
    }
}
