use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

pub struct DirectoryEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: Option<u64>,
}

const PATH_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'{')
    .add(b'}');

const FOLDER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;

const FILE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>"#;

const ARCHIVE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/></svg>"#;

const IMAGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/></svg>"#;

const VIDEO: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m16 13 5.223 3.482a.5.5 0 0 0 .777-.416V7.87a.5.5 0 0 0-.752-.432L16 10.5"/><rect x="2" y="6" width="14" height="12" rx="2"/></svg>"#;

const MUSIC: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>"#;

const CODE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m16 18 6-6-6-6"/><path d="m8 6-6 6 6 6"/></svg>"#;

const PACKAGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m7.5 4.27 9 5.15"/><path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/></svg>"#;

pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0".to_string();
    }
    let units = ["B", "KB", "MB", "GB"];
    let exponent = ((bytes as f64).ln() / 1024_f64.ln()).floor() as usize;
    let unit = match units.get(exponent) {
        Some(unit) => unit,
        None => return bytes.to_string(),
    };
    let value = bytes as f64 / 1024_f64.powi(exponent as i32);
    format!("{}{}", (value * 100.0).round() / 100.0, unit)
}

fn file_icon(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "exe" | "msi" | "dmg" | "pkg" | "deb" | "rpm" | "app" => PACKAGE,
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "ico" | "bmp" | "tiff" => IMAGE,
        "mp4" | "avi" | "mov" | "wmv" | "flv" | "webm" | "mkv" | "m4v" => VIDEO,
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" | "wma" => MUSIC,
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "bz" => ARCHIVE,
        "js" | "ts" | "jsx" | "tsx" | "html" | "css" | "json" | "xml" | "yaml" | "yml" | "py"
        | "java" | "cpp" | "c" | "h" | "php" | "rb" | "go" | "rs" | "sh" | "bat" | "ps1" | "md" => CODE,
        _ => FILE,
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn generate_directory_listing(items: &[DirectoryEntry], current_path: &str, parent_link: &str) -> String {
    let safe_current = escape_html(current_path);
    let safe_parent = utf8_percent_encode(parent_link, PATH_SET).to_string();
    let rows = items
        .iter()
        .map(|item| {
            let link = match current_path {
                "/" => format!("/{}", item.name),
                other => format!("{}/{}", other, item.name),
            };
            let display_name = match item.is_directory {
                true => format!("{}/", item.name),
                false => item.name.clone(),
            };
            let size = match item.is_directory {
                true => "-".to_string(),
                false => format_size(item.size.unwrap_or(0)),
            };
            let icon = match item.is_directory {
                true => FOLDER,
                false => file_icon(&item.name),
            };
            let safe_name = escape_html(&display_name);
            let safe_link = utf8_percent_encode(&link, PATH_SET).to_string();
            format!(
                r#"
              <tr class="hover:bg-blue-50 transition-all duration-300">
                <td class="px-6 py-4">
                  <a href="{safe_link}" class="text-black transition-all duration-300 hover:text-gray-700 font-medium flex items-center gap-3">
                    {icon}
                    <span>{safe_name}</span>
                  </a>
                </td>
                <td class="px-6 py-4 text-right text-gray-600 font-mono text-sm">{size}</td>
              </tr>"#
            )
        })
        .collect::<String>();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Index of {safe_current}</title>
    <script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"></script>
  </head>
  <body class="bg-gray-50 text-gray-900 min-h-screen">
    <div class="container mx-auto px-6 py-12 max-w-5xl">
      <div class="mb-8">
        <h1 class="text-4xl font-bold text-gray-900 mb-3">Index of {safe_current}</h1>
      </div>
      <div class="bg-white rounded-md shadow-lg border border-gray-200 overflow-hidden">
        <div class="overflow-x-auto">
          <table class="w-full">
            <thead class="bg-gradient-to-r from-gray-50 to-gray-100">
              <tr>
                <th class="px-6 py-4 text-left text-xs font-bold text-gray-700 uppercase tracking-widest">Name</th>
                <th class="px-6 py-4 text-right text-xs font-bold text-gray-700 uppercase tracking-widest">Size</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-gray-100">
              <tr class="hover:bg-blue-50 transition-all duration-300">
                <td class="px-6 py-4">
                  <a href="{safe_parent}" class="text-black hover:text-gray-700 transition-all duration-300 font-semibold flex items-center gap-3">
                    {FOLDER}
                    <span>Parent Directory</span>
                  </a>
                </td>
                <td class="px-6 py-4 text-right text-gray-500 font-mono text-sm">-</td>
              </tr>{rows}
            </tbody>
          </table>
        </div>
      </div>
      <div class="mt-10 text-center">
        <p class="text-gray-500 text-sm">Service powered by FastDL</p>
      </div>
    </div>
  </body>
</html>"#
    )
}
