use std::path::PathBuf;
use zip::write::SimpleFileOptions;
use crate::log_status;
use std::io::Cursor;

fn strip_prefix(p: &str, top_folder_name: &str) -> Option<String> {
    if p == top_folder_name {
        None
    } else {
        Some(p.strip_prefix(&format!("{top_folder_name}/")).map(|s| s.to_string()).unwrap_or(p.to_string()))
    }
}

pub fn read_files_from_zip(zip: &[u8], remove_top_folder: bool, dotfiles: &[&str]) -> Vec<(Option<String>, PathBuf, Vec<u8>)> {

    let cursor = Cursor::new(zip);
    let mut zip = match zip::ZipArchive::new(cursor) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let mut contents = Vec::new();
    for i in 0..zip.len() {

        let mut file = match zip.by_index(i) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let f_name = file.name();
        let file_n = PathBuf::from(f_name);
        let filename = file_n.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .and_then(|s| if s.starts_with(".") && !dotfiles.iter().any(|q| *q == s) { None } else { Some(s) });
        let filename = match filename {
            Some(s) => PathBuf::from(s),
            None => continue,
        };

        let parent_dir = file_n.parent().map(|s| s.as_os_str().to_string_lossy().to_string());
        let mut parent_dir = if parent_dir.clone().unwrap_or_default().is_empty() {
            None
        } else {
            parent_dir
        };

        let mut bytes = Cursor::new(Vec::new());
        let _ = match std::io::copy(&mut file, &mut bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };
        
        contents.push((parent_dir, filename, bytes.into_inner()));
    }

    let top_folder_name = match contents.iter().find(|s| s.0.clone().unwrap_or_default().is_empty()).map(|s| s.1.display().to_string()) {
        Some(s) => s,
        None => return contents,
    };

    contents = contents.into_iter().filter_map(|s| {
        if s.0.clone().unwrap_or_default().is_empty() || s.2.is_empty() {
            None
        } else {
            Some(s)
        }
    }).collect();

    if remove_top_folder {

        contents = contents.into_iter()
        .map(|(parent, filename, contents)| (parent.as_ref().and_then(|s| strip_prefix(s, &top_folder_name)), filename, contents))
        .collect();
    }
    contents
}

pub fn write_files_to_zip(files: Vec<(Option<String>, PathBuf, Vec<u8>)>) -> Vec<u8> {
    use std::io::Write;
    use zip::write::ZipWriter;

    let mut cursor = Cursor::new(Vec::new());

    {
        let mut zip = ZipWriter::new(&mut cursor);

        let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

        for (option_dir, _, _) in files.iter() {
            if let Some(dir) = option_dir {
                #[allow(deprecated)]
                let _ = zip.add_directory(dir, options);
            }
        }

        for (option_dir, path_buf, file_contents) in files {
            let path = path_buf.as_path();
            let name = path;

            let path_buf = if let Some(dir) = option_dir {
                PathBuf::from(format!("{}/{}", dir, name.display()))
            } else {
                PathBuf::from(format!("/{}", name.display()))
            };

            let path = path_buf.as_path();
            let name = path;

            total += file_contents.len();

            #[allow(deprecated)]
            if let Err(e) = zip.start_file_from_path(name, options) {
                log_status(&format!("{}", e.to_string()));
            }

            if let Err(e) = zip.write_all(&file_contents) {
                log_status(&format!("{}", e.to_string()));
            }
        }

        let _ = zip.finish();
    }

    cursor.into_inner()
}
