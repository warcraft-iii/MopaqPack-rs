use std::ffi::*;
use std::path::Path;
use std::ptr;
use stormlib_sys::*;

#[macro_use]
mod util;

mod constants;
pub use constants::*;

pub mod error;
use error::*;

/// MPQ archive
#[derive(Debug)]
pub struct Archive {
    handle: HANDLE,
}

impl Archive {
    /// Opens a MPQ archive
    pub fn open<P: AsRef<Path>>(path: P, flags: OpenArchiveFlags) -> Result<Self> {
        #[cfg(not(target_os = "windows"))]
        let cpath = {
            let pathstr = path.as_ref().to_str().ok_or_else(|| StormError::NonUtf8)?;
            CString::new(pathstr)?
        };
        #[cfg(target_os = "windows")]
        let cpath = {
            use widestring::U16CString;
            U16CString::from_os_str(path.as_ref())
                .map_err(|_| StormError::InteriorNul)?
                .into_vec()
        };
        let mut handle: HANDLE = ptr::null_mut();
        unsafe_try_call!(SFileOpenArchive(
            cpath.as_ptr(),
            0,
            flags.bits(),
            &mut handle as *mut HANDLE,
        ));
        Ok(Archive { handle })
    }

    pub fn create<P: AsRef<Path>>(path: P, filecount: usize, use_filelist: bool) -> Result<Self> {
        #[cfg(not(target_os = "windows"))]
        let cpath = {
            let pathstr = path.as_ref().to_str().ok_or_else(|| StormError::NonUtf8)?;
            CString::new(pathstr)?
        };
        #[cfg(target_os = "windows")]
        let cpath = {
            use widestring::U16CString;
            U16CString::from_os_str(path.as_ref())
                .map_err(|_| StormError::InteriorNul)?
                .into_vec()
        };
        let mut handle: HANDLE = ptr::null_mut();
        let flags = 0;
        let dwMpqVersion = (flags & MPQ_CREATE_ARCHIVE_VMASK) >> 24;
        let dwStreamFlags = 0;
        let mut dwFileFlags1 = if flags & MPQ_CREATE_LISTFILE != 0 {
            MPQ_FILE_DEFAULT_INTERNAL
        } else {
            0
        };
        let dwFileFlags2 = if flags & MPQ_CREATE_ATTRIBUTES != 0 {
            MPQ_FILE_DEFAULT_INTERNAL
        } else {
            0
        };
        let dwFileFlags3 = if flags & MPQ_CREATE_SIGNATURE != 0 {
            MPQ_FILE_DEFAULT_INTERNAL
        } else {
            0
        };
        let mut dwAttrFlags = if flags & MPQ_CREATE_ATTRIBUTES != 0 {
            MPQ_ATTRIBUTE_CRC32 | MPQ_ATTRIBUTE_FILETIME | MPQ_ATTRIBUTE_MD5
        } else {
            0
        };
        let dwSectorSize: u32 = if dwMpqVersion >= MPQ_FORMAT_VERSION_3 {
            0x4000
        } else {
            0x1000
        };
        let dwRawChunkSize = if dwMpqVersion >= MPQ_FORMAT_VERSION_4 {
            0x4000
        } else {
            0
        };
        let dwMaxFileCount = filecount;

        if dwMpqVersion >= MPQ_FORMAT_VERSION_3 && flags & MPQ_CREATE_ATTRIBUTES != 0 {
            dwAttrFlags |= MPQ_ATTRIBUTE_PATCH_BIT;
        }

        if use_filelist {
            dwFileFlags1 = MPQ_FILE_DEFAULT_INTERNAL;
        }

        let cbSize = ::std::mem::size_of::<_SFILE_CREATE_MPQ>() as u32;
        let mut ci = Box::new(_SFILE_CREATE_MPQ {
            cbSize,
            dwMpqVersion,
            pvUserData: ptr::null_mut(),
            cbUserData: 0,
            dwStreamFlags,
            dwFileFlags1,
            dwFileFlags2,
            dwFileFlags3,
            dwAttrFlags,
            dwSectorSize,
            dwRawChunkSize,
            dwMaxFileCount: dwMaxFileCount as u32,
        });

        unsafe_try_call!(SFileCreateArchive2(
            cpath.as_ptr(),
            &mut *ci,
            &mut handle as *mut HANDLE
        ));

        Ok(Archive { handle })
    }

    /// Quick check if the file exists within MPQ archive, without opening it
    pub fn has_file(&mut self, path: &str) -> Result<bool> {
        let cpath = CString::new(path)?;
        unsafe {
            let r = SFileHasFile(self.handle, cpath.as_ptr());
            let err = GetLastError();
            if !r && err != ERROR_FILE_NOT_FOUND {
                return Err(From::from(ErrorCode(err)));
            }
            Ok(r)
        }
    }

    /// Opens a file from MPQ archive
    pub fn open_file<'a>(&'a mut self, path: &str) -> Result<File<'a>> {
        let mut file_handle: HANDLE = ptr::null_mut();
        let cpath = CString::new(path)?;
        unsafe_try_call!(SFileOpenFileEx(
            self.handle,
            cpath.as_ptr(),
            0,
            &mut file_handle as *mut HANDLE
        ));
        Ok(File {
            archive: self,
            file_handle,
            size: None,
            need_reset: false,
        })
    }

    pub fn write_file(&self, file_name: &str, data: &[u8]) -> Result<bool> {
        let cpath = CString::new(file_name)?;
        let mut handle = ptr::null_mut();
        unsafe_try_call!(SFileCreateFile(
            self.handle,
            cpath.as_ptr(),
            0,
            data.len() as u32,
            0,
            MPQ_FILE_REPLACEEXISTING,
            &mut handle,
        ));
        unsafe_try_call!(SFileWriteFile(
            handle,
            data.as_ptr() as *const c_void,
            data.len() as u32,
            0
        ));
        unsafe_try_call!(SFileFinishFile(handle));
        Ok(true)
    }

    pub fn add_file(&mut self, path: &str, local_path: &str) -> Result<()> {
        #[cfg(not(target_os = "windows"))]
        let clocal_path = {
            let pathstr = local_path
                .as_ref()
                .to_str()
                .ok_or_else(|| StormError::NonUtf8)?;
            CString::new(pathstr)?
        };
        #[cfg(target_os = "windows")]
        let clocal_path = {
            use widestring::U16CString;
            U16CString::from_os_str(local_path)
                .map_err(|_| StormError::InteriorNul)?
                .into_vec()
        };
        let _ = self.remove_file(path);
        let cpath = CString::new(path)?;
        unsafe_try_call!(SFileAddFileEx(
            self.handle,
            clocal_path.as_ptr(),
            cpath.as_ptr(),
            MPQ_FILE_COMPRESS | MPQ_FILE_ENCRYPTED,
            MPQ_COMPRESSION_ZLIB,
            MPQ_COMPRESSION_NEXT_SAME,
        ));
        Ok(())
    }

    pub fn remove_file(&mut self, path: &str) -> Result<bool> {
        let cpath = CString::new(path)?;
        unsafe {
            let r = SFileRemoveFile(self.handle, cpath.as_ptr(), 0);
            let err = GetLastError();
            if !r && err != ERROR_FILE_NOT_FOUND {
                return Err(From::from(ErrorCode(err)));
            }
            Ok(r)
        }
    }

    pub fn compact(&mut self) -> Result<()> {
        unsafe_try_call!(SFileCompactArchive(self.handle, ptr::null_mut(), false));
        Ok(())
    }

    pub fn get_max_files(&mut self) -> Result<u32> {
        unsafe {
            let count = SFileGetMaxFileCount(self.handle);
            Ok(count)
        }
    }

    pub fn set_max_files(&mut self, count: u32) -> Result<()> {
        unsafe_try_call!(SFileSetMaxFileCount(self.handle, count));
        Ok(())
    }
}

impl std::ops::Drop for Archive {
    fn drop(&mut self) {
        unsafe {
            SFileCloseArchive(self.handle);
        }
    }
}

/// Opened file
#[derive(Debug)]
pub struct File<'a> {
    archive:     &'a Archive,
    file_handle: HANDLE,
    size:        Option<u64>,
    need_reset:  bool,
}

impl<'a> File<'a> {
    /// Retrieves a size of the file within archive
    pub fn get_size(&mut self) -> Result<u64> {
        if let Some(size) = self.size.clone() {
            Ok(size)
        } else {
            let mut high: DWORD = 0;
            let low = unsafe { SFileGetFileSize(self.file_handle, &mut high as *mut DWORD) };
            if low == SFILE_INVALID_SIZE {
                return Err(From::from(ErrorCode(unsafe { GetLastError() })));
            }
            let high = (high as u64) << 32;
            let size = high | (low as u64);
            self.size = Some(size);
            return Ok(size);
        }
    }

    /// Reads all data from the file
    pub fn read_all(&mut self) -> Result<Vec<u8>> {
        if self.need_reset {
            unsafe {
                if SFileSetFilePointer(self.file_handle, 0, ptr::null_mut(), 0)
                    == SFILE_INVALID_SIZE
                {
                    return Err(From::from(ErrorCode(GetLastError())));
                }
            }
        }

        let size = self.get_size()?;
        let mut buf = Vec::<u8>::with_capacity(size as usize);
        buf.resize(buf.capacity(), 0);
        let mut read: DWORD = 0;
        self.need_reset = true;
        unsafe_try_call!(SFileReadFile(
            self.file_handle,
            std::mem::transmute(buf.as_mut_ptr()),
            size as u32,
            &mut read as *mut DWORD,
            ptr::null_mut(),
        ));
        if (read as u64) < size {
            buf.truncate(read as usize);
        }
        Ok(buf)
    }
}

impl<'a> std::ops::Drop for File<'a> {
    fn drop(&mut self) {
        unsafe {
            SFileCloseFile(self.file_handle);
        }
    }
}

#[test]
fn test_read() {
    let mut archive = Archive::open(
        "../../samples/test_tft.w3x",
        OpenArchiveFlags::MPQ_OPEN_NO_LISTFILE | OpenArchiveFlags::MPQ_OPEN_NO_ATTRIBUTES,
    )
    .unwrap();

    assert_eq!(archive.has_file("invalid").unwrap(), false);
    assert_eq!(archive.has_file("war3map.j").unwrap(), true);
    let mut f = archive.open_file("war3map.j").unwrap();
    assert_eq!(f.get_size().unwrap(), 14115);
    assert_eq!(
        f.read_all().unwrap(),
        std::fs::read("../../samples/war3map.j").unwrap()
    );
}

#[cfg(target_os = "windows")]
#[test]
fn test_read_unicode() {
    use widestring::U16CString;
    use std::os::windows::ffi::OsStringExt;
    let mut archive = Archive::open(
        OsString::from_wide(
            &U16CString::from_str("../../samples/中文.w3x")
                .unwrap()
                .into_vec(),
        ),
        OpenArchiveFlags::MPQ_OPEN_NO_LISTFILE | OpenArchiveFlags::MPQ_OPEN_NO_ATTRIBUTES,
    )
    .unwrap();
    let mut f = archive.open_file("war3map.j").unwrap();
    assert_eq!(
        f.read_all().unwrap(),
        std::fs::read("../../samples/war3map.j").unwrap()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn test_read_utf8() {
    let mut archive = Archive::open(
        "../../samples/中文.w3x",
        OpenArchiveFlags::MPQ_OPEN_NO_LISTFILE | OpenArchiveFlags::MPQ_OPEN_NO_ATTRIBUTES,
    )
    .unwrap();
    let mut f = archive.open_file("war3map.j").unwrap();
    assert_eq!(
        f.read_all().unwrap(),
        std::fs::read("../../samples/war3map.j").unwrap()
    );
}
