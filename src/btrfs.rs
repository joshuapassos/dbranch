use crate::cli::Project;
use crate::config::Config;
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

#[derive(Debug, Clone)]
pub struct SubvolumeInfo {
    pub name: String,
    pub path: String,
    pub referenced_size: u64,
    pub exclusive_size: u64,
}

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
    // Img file path (e.g., /path/to/project/btrfs.img)
    img_path: PathBuf,
    // Mount point for the cow like filesystem (e.g., /mnt/projects/project_name)
    mount_point: String,
    size: u64,
}

impl BtrfsOperator {
    pub fn new(project: Project, config: Config) -> Self {
        let project_name = project.name.clone();

        let project_mount_point = format!("{}/{}", config.mount_point, project_name);

        Self {
            img_path: project.path.join("btrfs.img"),
            mount_point: project_mount_point.clone(),
            size: 1 * 1024 * 1024 * 1024 * 1024, // 1TB per project (adjustable)
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

    pub fn delete_img(&self) -> Result<()> {
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

        debug!("Creating main subvolume after mount");
        let main_subvolume = format!("{}/main", &self.mount_point);
        let output = std::process::Command::new("sudo")
            .args(&["btrfs", "subvolume", "create", &main_subvolume])
            .output()
            .unwrap();

        if !output.status.success() {
            return Err(AppError::Btrfs {
                message: format!(
                    "Failed to create main subvolume: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }
        debug!("Main subvolume created successfully: {}", main_subvolume);

        let data_dir = format!("{}/data", &main_subvolume);
        debug!("Creating data directory: {}", data_dir);
        let mkdir_output = std::process::Command::new("sudo")
            .arg("mkdir")
            .arg("-p")
            .arg(&data_dir)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to create data directory: {}", e),
            })?;

        if !mkdir_output.status.success() {
            return Err(AppError::FileSystem {
                message: format!(
                    "Failed to create data directory: stderr={} stdout={}",
                    String::from_utf8_lossy(&mkdir_output.stderr),
                    String::from_utf8_lossy(&mkdir_output.stdout)
                ),
            });
        }

        info!(
            "Successfully mounted disk at {} with main subvolume",
            self.mount_point
        );
        Ok(())
    }

    pub fn unmount_disk(&self) -> Result<(), error::AppError> {
        info!("Starting disk unmount process for {}", self.mount_point);
        Self::prompt_sudo_password().unwrap();

        debug!("Unmounting {}", self.mount_point);
        let output = std::process::Command::new("sudo")
            // It can cause btrfs filesystem corruption ~ https://stackoverflow.com/questions/7878707/how-to-unmount-a-busy-device
            .args(&["umount", "-l", self.mount_point.as_str()])
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

    pub fn cleanup_project_subvolume(&self, project_name: &str) -> Result<(), error::AppError> {
        info!("Starting cleanup of project subvolume: {}", project_name);
        Self::prompt_sudo_password().unwrap();

        let subvolume_path = format!("{}/{}", &self.mount_point, project_name);

        // Check if subvolume exists before trying to delete it
        if !self.subvolume_exists(project_name)? {
            debug!(
                "Subvolume {} does not exist, skipping deletion",
                project_name
            );
            return Ok(());
        }

        debug!("Deleting Btrfs subvolume: {}", subvolume_path);
        let output = std::process::Command::new("sudo")
            .arg("btrfs")
            .arg("subvolume")
            .arg("delete")
            .arg(&subvolume_path)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to delete subvolume: {}", e),
            })?;

        if output.status.success() {
            info!("Subvolume '{}' deleted successfully", project_name);
            Ok(())
        } else {
            Err(AppError::FileSystem {
                message: format!(
                    "Failed to delete subvolume '{}': stderr={} stdout={}",
                    project_name,
                    String::from_utf8_lossy(&output.stderr),
                    String::from_utf8_lossy(&output.stdout)
                ),
            })
        }
    }

    pub fn cleanup_disk(&self) -> Result<(), error::AppError> {
        info!("Starting disk cleanup process for {:?}", self.img_path);

        // First try to unmount the disk if it's mounted
        match self.unmount_disk() {
            Ok(_) => {
                debug!("Disk unmounted successfully");
            }
            Err(e) => {
                debug!("Failed to unmount disk (might not be mounted): {}", e);
                // Continue with cleanup even if unmount fails
            }
        }

        // Remove the disk image file if it exists
        if self.img_path.exists() {
            debug!("Removing disk image file: {:?}", self.img_path);
            match fs::remove_file(&self.img_path) {
                Ok(_) => {
                    info!("Disk image file removed successfully");
                }
                Err(e) => {
                    debug!("Failed to remove disk image file: {}", e);
                    return Err(AppError::FileSystem {
                        message: format!("Failed to remove disk image file: {}", e),
                    });
                }
            }
        } else {
            debug!("Disk image file does not exist, skipping removal");
        }

        info!("Disk cleanup completed successfully");
        Ok(())
    }

    pub fn create_snapshot(&self, snapshot_name: &str) -> Result<(), error::AppError> {
        debug!("Creating Btrfs snapshot: {}", snapshot_name);
        Self::prompt_sudo_password().unwrap();

        // Source is always the main subvolume of this version
        // TODO: change to snapshot from branches
        let source_subvolume = format!("{}/main", &self.mount_point);

        let target_snapshot = format!("{}/{}", &self.mount_point, snapshot_name);

        if !self.subvolume_exists("main")? {
            return Err(AppError::FileSystem {
                message: "Main subvolume not found - project may not be properly initialized"
                    .to_string(),
            });
        }

        debug!(
            "Running command: sudo btrfs subvolume snapshot {} {}",
            source_subvolume, target_snapshot
        );

        let output = std::process::Command::new("sudo")
            .arg("btrfs")
            .arg("subvolume")
            .arg("snapshot")
            .arg(source_subvolume)
            .arg(&target_snapshot)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to create Btrfs snapshot: {}", e),
            })?;

        if output.status.success() {
            debug!("Btrfs snapshot created successfully: {}", snapshot_name);
            info!("Snapshot '{}' created from main subvolume", snapshot_name);
            Ok(())
        } else {
            Err(AppError::FileSystem {
                message: format!(
                    "Failed to create Btrfs snapshot: stderr={} stdout={}",
                    String::from_utf8_lossy(&output.stderr),
                    String::from_utf8_lossy(&output.stdout)
                ),
            })
        }
    }

    fn subvolume_exists(&self, subvolume_name: &str) -> Result<bool, error::AppError> {
        let subvolume_path = format!("{}/{}", &self.mount_point, subvolume_name);
        debug!("Checking if subvolume exists: {}", subvolume_path);

        let output = std::process::Command::new("sudo")
            .arg("btrfs")
            .arg("subvolume")
            .arg("show")
            .arg(&subvolume_path)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to check subvolume existence: {}", e),
            })?;

        Ok(output.status.success())
    }

    fn list_subvolumes(&self) -> Result<Vec<String>, error::AppError> {
        debug!("Listing subvolumes in: {}", self.mount_point);

        let output = std::process::Command::new("sudo")
            .arg("btrfs")
            .arg("subvolume")
            .arg("list")
            .arg(&self.mount_point)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to list subvolumes: {}", e),
            })?;

        if !output.status.success() {
            return Err(AppError::FileSystem {
                message: format!(
                    "Failed to list subvolumes: stderr={}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let subvolumes: Vec<String> = stdout
            .lines()
            .filter_map(|line| {
                // Parse btrfs subvolume list output: "ID xxx gen xxx path subvolume_name"
                line.split_whitespace().last().map(|s| s.to_string())
            })
            .collect();

        Ok(subvolumes)
    }

    pub fn get_subvolume_info(
        &self,
        subvolume_name: &str,
    ) -> Result<SubvolumeInfo, error::AppError> {
        debug!("Getting info for subvolume: {}", subvolume_name);
        Self::prompt_sudo_password().unwrap();

        let subvolume_path = format!("{}/{}", &self.mount_point, subvolume_name);

        // Get quota info for the subvolume
        let output = std::process::Command::new("sudo")
            .arg("btrfs")
            .arg("qgroup")
            .arg("show")
            .arg("-r")
            .arg("-e")
            .arg("--raw")
            .arg(&self.mount_point)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to get subvolume quota info: {}", e),
            })?;

        if !output.status.success() {
            // If qgroups are not enabled, try to enable them first
            debug!("Qgroups might not be enabled, attempting to enable them");
            let _ = std::process::Command::new("sudo")
                .arg("btrfs")
                .arg("quota")
                .arg("enable")
                .arg(&self.mount_point)
                .output();

            // Try to get the sizes using du as a fallback
            return self.get_subvolume_size_fallback(subvolume_name);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut referenced_size: u64 = 0;
        let mut exclusive_size: u64 = 0;

        // Parse the qgroup output to find our subvolume
        for line in stdout.lines() {
            if line.contains(subvolume_name) || line.contains(&subvolume_path) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    referenced_size = parts[1].parse().unwrap_or(0);
                    exclusive_size = parts[2].parse().unwrap_or(0);
                    break;
                }
            }
        }

        // If we couldn't find it in qgroup output, use fallback
        if referenced_size == 0 && exclusive_size == 0 {
            return self.get_subvolume_size_fallback(subvolume_name);
        }

        Ok(SubvolumeInfo {
            name: subvolume_name.to_string(),
            path: subvolume_path,
            referenced_size,
            exclusive_size,
        })
    }

    fn get_subvolume_size_fallback(
        &self,
        subvolume_name: &str,
    ) -> Result<SubvolumeInfo, error::AppError> {
        debug!(
            "Using fallback method (du) to calculate subvolume size for: {}",
            subvolume_name
        );

        let subvolume_path = format!("{}/{}", &self.mount_point, subvolume_name);

        // Use du to get the size
        let output = std::process::Command::new("sudo")
            .arg("du")
            .arg("-sb")
            .arg(&subvolume_path)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to get subvolume size using du: {}", e),
            })?;

        if !output.status.success() {
            return Err(AppError::FileSystem {
                message: format!(
                    "Failed to get subvolume size: stderr={}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let size: u64 = stdout
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Ok(SubvolumeInfo {
            name: subvolume_name.to_string(),
            path: subvolume_path,
            referenced_size: size,
            exclusive_size: size, // In fallback mode, we can't determine exclusive size
        })
    }

    pub fn get_filesystem_info(&self) -> Result<(u64, u64, u64), error::AppError> {
        debug!("Getting filesystem info for: {}", self.mount_point);
        Self::prompt_sudo_password().unwrap();

        // Use df to get filesystem usage - simpler and more reliable
        let output = std::process::Command::new("df")
            .arg("-B1") // Output in bytes
            .arg(&self.mount_point)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to get filesystem info: {}", e),
            })?;

        if !output.status.success() {
            // Fallback to du if df fails
            return self.get_filesystem_info_fallback();
        }

        // Parse df output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();

        if lines.len() < 2 {
            return self.get_filesystem_info_fallback();
        }

        // Parse the second line (first line is header)
        let parts: Vec<&str> = lines[1].split_whitespace().collect();
        if parts.len() < 4 {
            return self.get_filesystem_info_fallback();
        }

        // df output format: Filesystem 1K-blocks Used Available Use% Mounted
        let total_bytes = parts[1].parse::<u64>().unwrap_or(self.size);
        let used_bytes = parts[2].parse::<u64>().unwrap_or(0);
        let available_bytes = parts[3].parse::<u64>().unwrap_or(0);

        Ok((total_bytes, used_bytes, available_bytes))
    }

    fn get_filesystem_info_fallback(&self) -> Result<(u64, u64, u64), error::AppError> {
        debug!("Using fallback method (du) to calculate filesystem usage");

        // Use du to get actual used space for all subvolumes
        let output = std::process::Command::new("sudo")
            .arg("du")
            .arg("-sb")
            .arg(&self.mount_point)
            .output()
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to get filesystem usage with du: {}", e),
            })?;

        let used_bytes = if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        } else {
            0
        };

        let total_bytes = self.size;
        let available_bytes = if used_bytes < total_bytes {
            total_bytes - used_bytes
        } else {
            0
        };

        Ok((total_bytes, used_bytes, available_bytes))
    }

    pub fn get_all_subvolumes_info(&self) -> Result<Vec<SubvolumeInfo>, error::AppError> {
        debug!("Getting info for all subvolumes");

        let subvolumes = self.list_subvolumes()?;
        let mut infos = Vec::new();

        for subvolume in subvolumes {
            match self.get_subvolume_info(&subvolume) {
                Ok(info) => infos.push(info),
                Err(e) => {
                    debug!("Failed to get info for subvolume {}: {}", subvolume, e);
                    // Continue with other subvolumes
                }
            }
        }

        Ok(infos)
    }
}
