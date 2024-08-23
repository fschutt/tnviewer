use std::path::PathBuf;

use zip::write::SimpleFileOptions;

pub fn write_files_to_zip(files: &[(Option<String>, PathBuf, Vec<u8>)]) -> Vec<u8> {
    use std::io::Cursor;
    use std::io::Write;
    use zip::write::{FileOptions, ZipWriter};

    let mut cursor = Cursor::new(Vec::new());

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

            #[allow(deprecated)]
            let e = zip.start_file_from_path(name, options);

            let e = zip.write_all(&file_contents);
        }

        let _ = zip.finish();
    }

    cursor.into_inner()
}
