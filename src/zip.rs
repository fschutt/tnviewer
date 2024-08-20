use std::path::PathBuf;

use zip::write::SimpleFileOptions;

pub fn write_files_to_zip(files: &[(Option<String>, PathBuf, Vec<u8>)]) -> Vec<u8> {
    use std::io::Cursor;
    use std::io::Write;
    use zip::write::{FileOptions, ZipWriter};

    let mut cursor = Cursor::new(Vec::new());

    web_sys::console::log_1(&format!("1").as_str().into());

    {
        let mut zip = ZipWriter::new(&mut cursor);

        let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

        for (option_dir, _, _) in files.iter() {
            if let Some(dir) = option_dir {
                #[allow(deprecated)]
                let _ = zip.add_directory(dir, options);
            }
        }

        web_sys::console::log_1(&format!("directories added!").as_str().into());

        for (option_dir, path_buf, file_contents) in files.iter() {
            let path = path_buf.as_path();
            let name = path;

            let path_buf = if let Some(dir) = option_dir {
                PathBuf::from(format!("{}/{}", dir, name.display()))
            } else {
                PathBuf::from(format!("/{}", name.display()))
            };

            let path = path_buf.as_path();
            let name = path;

            web_sys::console::log_1(&format!("adding file {}: {} bytes", name.display(), file_contents.len()).as_str().into());

            #[allow(deprecated)]
            let e = zip.start_file_from_path(name, options);
            web_sys::console::log_1(&format!("starting file {}: {e:?}", name.display()).as_str().into());

            let e = zip.write_all(&file_contents);
            web_sys::console::log_1(&format!("wrote file {}: {e:?}", name.display()).as_str().into());
        }

        web_sys::console::log_1(&"finishing...".into());

        let _ = zip.finish();
    }

    web_sys::console::log_1(&"zip finished!".into());

    cursor.into_inner()
}
