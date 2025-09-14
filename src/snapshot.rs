use tracing::debug;

use std::{fs, path::Path};

use crate::{
    copy_ref::{CopyRef, CopyRefOperator},
    error::AppError,
};

pub fn snapshot(src: &Path, dst: &Path) -> Result<(), AppError> {
    debug!("Creating snapshot from {:?} to {:?}", src, dst);
    let src = src.to_path_buf();
    let dst = dst.to_path_buf();

    if !dst.exists() {
        fs::create_dir_all(&dst).map_err(|e| AppError::FileSystem {
            message: format!("Failed to create directory {:?}: {}", dst, e),
        })?;
    }

    for entry in fs::read_dir(src.clone())
        .map_err(|e| AppError::FileSystem {
            message: format!("Failed to read directory {:?}: {}", src, e),
        })
        .unwrap()
    {
        match entry {
            Ok(entry) => {
                if entry.path().is_dir() {
                    let new_dst = dst.join(entry.file_name());
                    fs::create_dir_all(&new_dst).map_err(|e| AppError::FileSystem {
                        message: format!("Failed to create directory {:?}: {}", new_dst, e),
                    })?;
                    snapshot(&entry.path(), &new_dst)?;
                } else {
                    let src_file =
                        fs::File::open(entry.path()).map_err(|e| AppError::FileSystem {
                            message: format!(
                                "Failed to open source file {:?}: {}",
                                entry.path(),
                                e
                            ),
                        })?;
                    let dst_file_path = dst.join(entry.file_name());
                    let dst_file =
                        fs::File::create(&dst_file_path).map_err(|e| AppError::FileSystem {
                            message: format!(
                                "Failed to create destination file {:?}: {}",
                                dst_file_path, e
                            ),
                        })?;

                    let operator = CopyRefOperator::new();
                    operator.copy_ref(&src_file, &dst_file)?;
                }
            }
            Err(err) => {
                return Err(AppError::FileSystem {
                    message: format!("Failed to read directory entry: {}", err),
                });
            }
        }
    }

    Ok(())
}
