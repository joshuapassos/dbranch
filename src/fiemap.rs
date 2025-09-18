use std::fs::File;

use crate::error::AppError;
// from https://github.com/torvalds/linux/blob/cbf658dd09419f1ef9de11b9604e950bdd5c170b/include/uapi/linux/fiemap.h

#[repr(u32)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FiemapFlags {
    Last = 0x00000001,        // Last extent in file
    Unknown = 0x00000002,     // Data location unknown
    Delalloc = 0x00000004,    // Location still pending
    Encoded = 0x00000008,     // Data compressed/encrypted
    DataCrypted = 0x00000080, // Data is encrypted
    NotAligned = 0x00000100,  // Extent not aligned
    DataInline = 0x00000200,  // Data mixed with metadata
    DataTail = 0x00000400,    // Multiple files in block
    Unwritten = 0x00000800,   // Space allocated, no data
    Merged = 0x00001000,      // File does not natively support extents
    Shared = 0x00002000,      // Space shared with other files (reflink/CoW)
}

impl FiemapFlags {
    pub fn from_bits(flags: u32) -> Vec<FiemapFlags> {
        let mut result = Vec::new();
        if flags & Self::Last as u32 != 0 {
            result.push(Self::Last);
        }
        if flags & Self::Unknown as u32 != 0 {
            result.push(Self::Unknown);
        }
        if flags & Self::Delalloc as u32 != 0 {
            result.push(Self::Delalloc);
        }
        if flags & Self::Encoded as u32 != 0 {
            result.push(Self::Encoded);
        }
        if flags & Self::DataCrypted as u32 != 0 {
            result.push(Self::DataCrypted);
        }
        if flags & Self::NotAligned as u32 != 0 {
            result.push(Self::NotAligned);
        }
        if flags & Self::DataInline as u32 != 0 {
            result.push(Self::DataInline);
        }
        if flags & Self::DataTail as u32 != 0 {
            result.push(Self::DataTail);
        }
        if flags & Self::Unwritten as u32 != 0 {
            result.push(Self::Unwritten);
        }
        if flags & Self::Merged as u32 != 0 {
            result.push(Self::Merged);
        }
        if flags & Self::Shared as u32 != 0 {
            result.push(Self::Shared);
        }
        result
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct FiemapExtent {
    // byte offset of the extent in the file
    pub fe_logical: u64,
    // byte offset of extent on disk
    pub fe_physical: u64,
    // length in bytes for this extent
    pub fe_length: u64,

    fe_reserved64: [u64; 2],
    // flags for this extent
    pub fe_flags: u32,

    fe_reserved32: [u32; 3],
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct FiemapRequest {
    //  byte offset (inclusive) at which to start mapping (in)
    fm_start: u64,
    // logical length of mapping which userspace wants (in)
    fm_length: u64,
    // FIEMAP_FLAG_* flags for request (in/out)
    fm_flags: u32,
    // number of extents that were mapped (out)
    fm_mapped_extents: u32,
    // size of fm_extents array (in)
    fm_extent_count: u32,
    /* private: */
    fm_reserved: u32,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct FiemapRequestFull {
    pub request: FiemapRequest,
    /// array of mapped extents (out)
    /// 32 is the most that `Default` gives us ootb.
    pub fm_extents: [FiemapExtent; 32],
}

#[derive(Debug)]
pub struct Fiemap {
    pub extent: FiemapExtent,
    pub flags: Vec<FiemapFlags>,
}

pub fn check_file(f: File) -> Result<Vec<Fiemap>, AppError> {
    use std::os::fd::AsRawFd;

    let file_size = f.metadata().unwrap().len();
    const FS_IOC_FIEMAP: u64 = nix::libc::_IOWR::<FiemapRequest>(0x66, 11);

    let mut all_extents: Vec<Fiemap> = Vec::new();
    let mut current_offset: u64 = 0;

    loop {
        let mut fr = Box::new(FiemapRequestFull::default());
        fr.request.fm_start = current_offset;
        fr.request.fm_length = file_size - current_offset;
        fr.request.fm_flags = 0;
        fr.request.fm_extent_count = 32;

        let ret = unsafe { nix::libc::ioctl(f.as_raw_fd(), FS_IOC_FIEMAP, &mut *fr) };

        if ret == -1 {
            let errno = std::io::Error::last_os_error();
            eprintln!(
                "FIEMAP ioctl failed: {} (errno: {})",
                errno,
                errno.raw_os_error().unwrap()
            );
            return Err(AppError::FileSystem {
                message: format!("FIEMAP ioctl failed: {}", errno),
            });
        }

        if fr.request.fm_mapped_extents == 0 {
            break;
        }

        let mut found_last = false;
        for i in 0..fr.request.fm_mapped_extents as usize {
            let extent = fr.fm_extents[i];
            all_extents.push(Fiemap {
                extent,
                flags: FiemapFlags::from_bits(extent.fe_flags),
            });

            if extent.fe_flags & FiemapFlags::Last as u32 != 0 {
                found_last = true;
                break;
            }

            current_offset = extent.fe_logical + extent.fe_length;
        }

        if found_last || fr.request.fm_mapped_extents < 32 {
            break;
        }
    }

    Ok(all_extents)
}
