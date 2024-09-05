use std::path::PathBuf;
use crate::log_status;

use zip::write::SimpleFileOptions;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

struct CountingAllocator<A> {
    inner: A,
    allocated_now: AtomicIsize,
}

impl<A> CountingAllocator<A> {
    const fn new(inner: A) -> Self {
        Self {
            inner,
            allocated_now: AtomicIsize::new(0),
        }
    }

    fn allocated_now(&self) -> usize {
        self.allocated_now
            .load(Ordering::Relaxed)
            .try_into()
            .unwrap_or(0)
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocated_now
            .fetch_add(layout.size() as isize, Ordering::Relaxed);
        self.inner.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.allocated_now
            .fetch_sub(layout.size() as isize, Ordering::Relaxed);
        self.inner.dealloc(ptr, layout);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.allocated_now
            .fetch_add(layout.size() as isize, Ordering::Relaxed);
        self.inner.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.allocated_now.fetch_add(
            new_size as isize - layout.size() as isize,
            Ordering::Relaxed,
        );
        self.inner.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator<System> = CountingAllocator::new(System);

fn allocated() -> isize {
    ALLOCATOR.allocated_now.load(Ordering::Relaxed)
}

pub fn write_files_to_zip(files: &[(Option<String>, PathBuf, Vec<u8>)]) -> Vec<u8> {
    use std::io::Cursor;
    use std::io::Write;
    use zip::write::{FileOptions, ZipWriter};

    let mut cursor = Cursor::new(Vec::new());

    let mut total = 0;

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

        log_status("1");

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

            total += file_contents.len();
            log_status(&format!("encoding {} {} bytes (total = {total}), alloc = {}", name.display(), file_contents.len(), allocated()));

            #[allow(deprecated)]
            if let Err(e) = zip.start_file_from_path(name, options) {
                log_status(&format!("{}", e.to_string()));
            }

            log_status(&format!("2 allocated: {}", allocated()));

            if let Err(e) = zip.write_all(&file_contents) {
                log_status(&format!("{}", e.to_string()));
            }
            log_status(&format!("3 allocated: {}", allocated()));
        }

        let _ = zip.finish();
    }

    cursor.into_inner()
}
