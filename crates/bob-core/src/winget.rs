use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WingetRow {
    pub name: String,
    pub id: String,
    pub version: String,
    pub available: String,
    pub source: String,
}

pub fn parse_winget_table(text: &str) -> Vec<WingetRow> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(sep) = lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.len() >= 8 && trimmed.chars().all(|ch| ch == '-' || ch == ' ') && trimmed.contains("---")
    }) else {
        return Vec::new();
    };
    let header = lines[..sep]
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
        .copied()
        .unwrap_or("");
    let columns: Vec<String> = split_cols(header).into_iter().map(|name| column_key(&name)).collect();
    let mut rows = Vec::new();
    for line in lines.iter().skip(sep + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('-') {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if lower.starts_with("no package") || lower.starts_with("nenhum") || lower.starts_with("name ") {
            continue;
        }
        let cells = split_cols(line);
        if cells.is_empty() {
            continue;
        }
        let mut row = WingetRow {
            name: String::new(),
            id: String::new(),
            version: String::new(),
            available: String::new(),
            source: String::new(),
        };
        for (index, cell) in cells.iter().enumerate() {
            let key = columns.get(index).map(String::as_str).unwrap_or("other");
            match key {
                "name" => row.name = cell.clone(),
                "id" => row.id = cell.clone(),
                "version" => row.version = cell.clone(),
                "available" => row.available = cell.clone(),
                "source" => row.source = cell.clone(),
                _ => {}
            }
        }
        if row.name.is_empty() && !cells.is_empty() {
            row.name = cells[0].clone();
        }
        if row.id.is_empty() && cells.len() > 1 {
            row.id = cells[1].clone();
        }
        if !row.name.is_empty() {
            rows.push(row);
        }
    }
    rows
}

fn column_key(name: &str) -> String {
    let n = name.trim().to_lowercase();
    if n.starts_with("nome") || n.starts_with("name") {
        "name".into()
    } else if n == "id" {
        "id".into()
    } else if n.starts_with("vers") {
        "version".into()
    } else if n.starts_with("dispon") || n.starts_with("available") {
        "available".into()
    } else if n.starts_with("origem") || n.starts_with("source") {
        "source".into()
    } else {
        "other".into()
    }
}

fn split_cols(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut cols = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ' ' {
            let mut j = i;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            if j - i >= 2 {
                let text = buf.trim().to_string();
                if !text.is_empty() {
                    cols.push(text);
                }
                buf.clear();
                i = j;
                continue;
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    let text = buf.trim().to_string();
    if !text.is_empty() {
        cols.push(text);
    }
    cols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_english_and_portuguese() {
        let english = "\
Name                  Id                 Version   Source\n\
-----------------------------------------------------------\n\
VLC media player      VideoLAN.VLC       3.0.21    winget\n\
";
        let rows = parse_winget_table(english);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "VideoLAN.VLC");
        assert_eq!(rows[0].version, "3.0.21");

        let portuguese = "\
Nome                  Id                 Versão    Disponível  Origem\n\
---------------------------------------------------------------------\n\
Firefox               Mozilla.Firefox    134.0     135.0       winget\n\
";
        let rows = parse_winget_table(portuguese);
        assert_eq!(rows[0].name, "Firefox");
        assert_eq!(rows[0].available, "135.0");
        assert_eq!(rows[0].source, "winget");
    }
}
