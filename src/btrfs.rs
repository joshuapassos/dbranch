use crate::error;
use crate::error::AppError;
use anyhow::Result;
use regex::Regex;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;

use tracing::debug;
use tracing::info;

fn find_device_by_path(input: &str, target_path: &str) -> Option<String> {
    debug!("Searching for device with path: {}", target_path);
    let re =
        Regex::new(r"^(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\S+)\s+(\d+)\s+(\d+)$").unwrap();

    for line in input.lines().skip(1) {
        if let Some(caps) = re.captures(line) {
            let device = caps.get(1)?.as_str();
            let path = caps.get(6)?.as_str();

            if path.ends_with(target_path) {
                debug!("Found device {} for path {}", device, target_path);
                return Some(device.to_string());
            }
        }
    }
    debug!("No device found for path: {}", target_path);
    None
}

#[derive(Debug)]
pub struct BtrfsOperator {
    img_path: PathBuf,
    mount_point: String,
    size: u64,
}

impl BtrfsOperator {
    pub fn new(img_path: PathBuf, mount_point: String, size: u64) -> Self {
        debug!(
            "Creating BtrfsOperator: img={:?}, mount={}, size={} bytes",
            img_path, mount_point, size
        );
        Self {
            img_path,
            mount_point,
            size,
        }
    }

    pub fn prompt_sudo_password() -> Result<(), error::AppError> {
        // Check if we already have sudo privileges
        let check_output = std::process::Command::new("sudo")
            .args(&["-n", "echo", "sudo check"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| error::AppError::Permission {
                message: format!("Failed to check sudo status: {}", e),
            })?;

        if check_output.status.success() {
            debug!("Sudo privileges already available");
            return Ok(());
        }

        // Prompt for password
        print!("🔐 To continue, enter your sudo password: ");
        std::io::stdout().flush().map_err(|e| AppError::Internal {
            message: format!("Failed to flush stdout: {}", e),
        })?;

        // Use sudo -v to validate password with proper terminal
        let validate_status = std::process::Command::new("sudo")
            .args(&["-v"])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| AppError::Auth {
                message: format!("Failed to validate sudo password: {}", e),
            })?;

        if !validate_status.success() {
            return Err(AppError::Auth {
                message: "Incorrect sudo password or access denied".to_string(),
            });
        }

        info!("Sudo password validated successfully");
        Ok(())
    }

    pub fn reserve_space(&self) -> Result<()> {
        info!("Reserving disk space of {} bytes for image", self.size);
        debug!("Image path: {:?}", self.img_path);

        match fs::create_dir(Path::new(self.img_path.parent().unwrap().as_os_str())) {
            Ok(_) => {
                info!("Project directory created successfully.");
            }
            Err(e) => {
                debug!(
                    "Failed to create project directory: {:?} - {} Ignoring...",
                    Path::new(&self.img_path.parent().unwrap()),
                    e
                );
            }
        }

        debug!("Creating sparse file at {:?}", self.img_path);
        let file = File::options()
            .write(true)
            .create(true)
            .open(&self.img_path)
            .unwrap();

        file.set_len(self.size).unwrap();
        info!("Successfully reserved {} bytes of disk space", self.size);
        Ok(())
    }

    pub fn release_space(&self) -> Result<()> {
        info!("Releasing disk space for image at {:?}", self.img_path);
        let file = File::options().write(true).open(&self.img_path).unwrap();
        file.set_len(0).unwrap();
        fs::remove_file(&self.img_path)?;
        debug!("Disk space released successfully");
        Ok(())
    }

    pub fn mount_disk(&mut self) -> Result<(), error::AppError> {
        info!("Starting disk mount process for {:?}", self.img_path);
        Self::prompt_sudo_password().unwrap();

        debug!("Creating loop device for image");
        let output = std::process::Command::new("sudo")
            .args(&["losetup", "-f", "--show", &self.img_path.to_str().unwrap()])
            .output()
            .unwrap();

        if !output.status.success() {
            return Err(AppError::DiskMount {
                message: format!(
                    "Failed to create loop device: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let loop_device = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info!(target: "btrfs", "Loop device created: {}", loop_device);

        debug!("Formatting loop device {} as Btrfs", loop_device);
        let output = std::process::Command::new("sudo")
            .args(&["mkfs.btrfs", "-f", &loop_device])
            .output()
            .unwrap();

        if !output.status.success() {
            return Err(AppError::Btrfs {
                message: format!(
                    "Failed to format loop device as Btrfs: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        debug!("Creating mount point directory at {}", self.mount_point);
        let output = std::process::Command::new("sudo")
            .args(&["mkdir", "-p", self.mount_point.as_str()])
            .output()
            .unwrap();

        if !output.status.success() {
            return Err(AppError::FileSystem {
                message: format!(
                    "Failed to create mount point directory: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }
        debug!("Mount point directory created successfully");

        debug!("Mounting {} to {}", loop_device, self.mount_point);
        let output = std::process::Command::new("sudo")
            .args(&["mount", &loop_device, self.mount_point.as_str()])
            .output()
            .unwrap();

        if !output.status.success() {
            return Err(AppError::DiskMount {
                message: format!(
                    "Failed to mount loop device: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        info!("Successfully mounted disk at {}", self.mount_point);
        Ok(())
    }

    pub fn unmount_disk(&self) -> Result<(), error::AppError> {
        info!("Starting disk unmount process for {}", self.mount_point);
        Self::prompt_sudo_password().unwrap();

        debug!("Unmounting {}", self.mount_point);
        let output = std::process::Command::new("sudo")
            .args(&["umount", self.mount_point.as_str()])
            .output()
            .unwrap();
        if !output.status.success() {
            if String::from_utf8(output.stderr.clone())
                .unwrap()
                .contains("not mounted")
            {
                debug!("Disk already unmounted, continuing...");
            } else {
                return Err(AppError::DiskMount {
                    message: format!(
                        "Failed to unmount loop device: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                });
            }
        }

        debug!("Listing loop devices to find device for detachment");
        let output = std::process::Command::new("sudo")
            .args(&["losetup"])
            .output()
            .unwrap();
        if !output.status.success() {
            return Err(AppError::DiskMount {
                message: format!(
                    "Failed to list loop devices: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let device = find_device_by_path(
            String::from_utf8(output.stdout).unwrap().as_str(),
            &self.img_path.to_str().unwrap(),
        );

        let device_to_detach = device.or(Some("--all".into())).unwrap();
        debug!("Detaching loop device: {}", device_to_detach);
        let output = std::process::Command::new("sudo")
            .args(&["losetup", "-d", device_to_detach.as_str()])
            .output()
            .unwrap();
        if !output.status.success() {
            return Err(AppError::DiskMount {
                message: format!(
                    "Failed to detach loop device: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }
        debug!("Loop device detached successfully");

        self.release_space().unwrap();
        info!("Disk unmount process completed successfully");

        Ok(())
    }

    pub fn check_btrfs(&self) -> Result<(), String> {
        debug!("Checking for Btrfs installation");
        let output = std::process::Command::new("btrfs")
            .arg("version")
            .output()
            .map_err(|e| e.to_string())?;

        info!(target: "btrfs", "{}", String::from_utf8_lossy(&output.stdout).lines().next().unwrap());

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into())
        }
    }

    // pub fn create_snapshot(&self, snapshot_name: &str) -> Result<(), String> {
    //     let output = std::process::Command::new("btrfs")
    //         .arg("subvolume")
    //         .arg("snapshot")
    //         .arg(self.mount_point.as_ref().unwrap())
    //         .arg(snapshot_name)
    //         .output()
    //         .map_err(|e| e.to_string())?;

    //     if output.status.success() {
    //         Ok(())
    //     } else {
    //         Err(String::from_utf8_lossy(&output.stderr).into())
    //     }
    // }
}
