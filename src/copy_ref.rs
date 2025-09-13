use std::{
    fs::File,
    os::raw::{c_char, c_int},
};

use crate::error;

trait CopyRef {
    fn copy_ref(&self, src: &File, dest: &File) -> Result<(), error::AppError>;
}

pub struct CopyRefOperator {}

impl CopyRefOperator {
    pub fn new() -> Self {
        Self {}
    }
}

unsafe extern "C" {
    // http://www.manpagez.com/man/2/clonefileat/
    fn clonefile(src: *const c_char, dest: *const c_char, flags: c_int) -> c_int;
}

impl CopyRef for CopyRefOperator {
    #[cfg(target_os = "linux")]
    fn copy_ref(&self, src: &File, dest: &File) -> Result<(), error::AppError> {
        use std::os::fd::AsFd;

        let info = src.metadata().unwrap().len() as usize;
        // https://man7.org/linux/man-pages/man2/copy_file_range.2.html
        rustix::fs::copy_file_range(src.as_fd(), Some(&mut 0), dest.as_fd(), Some(&mut 0), info)
            .map_err(|err| error::AppError::FileSystem {
                message: format!("Failed to copy ref from {:?} to {:?}: {}", src, dest, err),
            })
            .unwrap();
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn copy_ref(&self, _src: &File, _dest: &File) -> Result<(), error::AppError> {
        let r = unsafe { clonefile(_src, _dest) };
        if r == -1 {
            let err = io::Error::last_os_error();
            return Err(error::AppError::FileSystem {
                message: format!("Failed to copy ref from {:?} to {:?}: {}", _src, _dest, err),
            });
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn copy_ref(&self, src: &File, dest: &File) -> Result<(), error::AppError> {
        Err(error::AppError::FileSystem {
            message: format!("copy_file_range not supported on this platform"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{BufWriter, Write},
    };

    #[test]
    fn test_copy_ref_basic() {
        let operator = CopyRefOperator::new();

        let dir = std::path::Path::new("./test_data");
        let src_path = dir.join("source.txt");
        let dest_path = dir.join("dest.txt");
        const MSG: &str = "Eu gosto de memes\n";
        const FILE_SIZE: usize = 2 * 1024 * 1024; // 2MB in bytes

        let mut writer = BufWriter::new(File::create(&src_path).unwrap());
        let chunk_size = MSG.len();
        let mut written = 0;

        while written < FILE_SIZE {
            let remaining = FILE_SIZE - written;
            if remaining < chunk_size {
                writer.write_all(&MSG.as_bytes()[..remaining]).unwrap();
                written += remaining;
            } else {
                writer.write_all(MSG.as_bytes()).unwrap();
                written += chunk_size;
            }
        }

        writer.flush().unwrap();

        let src = File::open(&src_path).unwrap();
        let dest = File::create(&dest_path).unwrap();

        let result = operator.copy_ref(&src, &dest);

        assert!(result.is_ok(), "Failed to copy file: {:?}", result);

        let src_content = fs::read_to_string(&src_path).unwrap();
        let dest_content = fs::read_to_string(&dest_path).unwrap();

        assert_eq!(
            src_content, dest_content,
            "Contents do not match after copy_ref"
        );

        //TODO: check physical_offset and shared flag
    }
}
