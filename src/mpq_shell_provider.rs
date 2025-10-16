use crate::{CLSID_MPQ_FOLDER, DLL_LOCK_COUNT, ProviderState, archive::MpqArchiveDescriptor, archive::MpqEntry};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use windows_implement::implement;

use crate::archive::MpqArchiveError;
use crate::log::log;
use crate::utils::guid::GuidExt;
use windows::Win32::Foundation::{E_FAIL, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER, S_FALSE, STG_E_ACCESSDENIED, STG_E_FILENOTFOUND};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::System::Com::StructuredStorage::{IEnumSTATSTG, IEnumSTATSTG_Impl, IStorage_Impl, STGMOVE};
use windows::Win32::System::Com::{CoTaskMemAlloc, IPersistFile_Impl, ISequentialStream, IStream, STATFLAG_NONAME, STATSTG, STGM, STGM_READ, STGTY_STORAGE, STGTY_STREAM, STREAM_SEEK_SET};
use windows::Win32::UI::Shell::PropertiesSystem::{IInitializeWithFile_Impl, IInitializeWithStream_Impl};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::Shell::{IInitializeWithItem_Impl, IShellItem, IThumbnailProvider_Impl, SIGDN_FILESYSPATH, WTS_ALPHATYPE};
use windows::core::{self as wcore, Error, Result as WinResult};
use windows_core::{Interface, PCWSTR, PWSTR};

#[implement(windows::Win32::UI::Shell::IThumbnailProvider, windows::Win32::UI::Shell::IInitializeWithItem, windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithStream, windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithFile, windows::Win32::System::Com::StructuredStorage::IStorage, windows::Win32::System::Com::IPersistFile)]
pub struct MpqShellProvider {
    state: Mutex<ProviderState>,
}

impl MpqShellProvider {
    pub fn new() -> Self {
        DLL_LOCK_COUNT.fetch_add(1, Ordering::SeqCst);
        log("MpqShellProvider::new");
        Self { state: Mutex::new(ProviderState::default()) }
    }

    fn load_descriptor_from_path(path: &str) -> wcore::Result<Arc<MpqArchiveDescriptor>> {
        log(format!("MpqShellProvider::load_descriptor_from_path path={}", path));
        MpqArchiveDescriptor::load_from_path(path)
            .map(Arc::new)
            .map_err(|err| Self::archive_err_to_hresult("load_from_path", err))
    }

    fn load_descriptor_from_bytes(bytes: &Arc<[u8]>) -> wcore::Result<Arc<MpqArchiveDescriptor>> {
        log(format!("MpqShellProvider::load_descriptor_from_bytes len={}", bytes.len()));
        let descriptor = MpqArchiveDescriptor::load_from_bytes(bytes.clone()).map_err(|err| Self::archive_err_to_hresult("load_from_bytes", err))?;
        Ok(Arc::new(descriptor))
    }

    fn archive_err_to_hresult(stage: &str, err: MpqArchiveError) -> Error {
        log(format!("MPQ archive load failed at {}: {}", stage, err));
        Error::from(E_FAIL)
    }
}

impl Drop for MpqShellProvider {
    fn drop(&mut self) {
        DLL_LOCK_COUNT.fetch_sub(1, Ordering::SeqCst);
        log("MpqShellProvider::drop");
    }
}

impl MpqShellProvider_Impl {
    fn set_state_from_descriptor(&self, path: Option<String>, stream_data: Option<Arc<[u8]>>, descriptor: Arc<MpqArchiveDescriptor>) {
        let mut st = self.state.lock().unwrap();
        st.path_utf8 = path;
        st.stream_data = stream_data;
        st.archive = descriptor;
        log("MpqShellProvider state updated");
    }

    fn name_from_pwcs(pwcs: &PCWSTR) -> String {
        unsafe {
            if pwcs.is_null() || pwcs.0.is_null() {
                String::new()
            } else {
                pwcs.to_string().unwrap_or_default()
            }
        }
    }

    fn descriptor(&self) -> Arc<MpqArchiveDescriptor> {
        let st = self.state.lock().unwrap();
        st.archive.clone()
    }

    fn friendly_storage_name(&self) -> String {
        let st = self.state.lock().unwrap();
        st.path_utf8
            .clone()
            .unwrap_or_else(|| "MPQ archive (stream source)".to_string())
    }
}

// ============================
// IInitialize* implementations
// ============================

impl IInitializeWithItem_Impl for MpqShellProvider_Impl {
    #[allow(non_snake_case)]
    fn Initialize(&self, psi: windows::core::Ref<'_, IShellItem>, _grf_mode: u32) -> wcore::Result<()> {
        unsafe {
            let item: &IShellItem = psi.ok()?;
            let pw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            if pw.is_null() {
                return Err(Error::from(E_FAIL));
            }
            let path = widestring::U16CStr::from_ptr_str(pw.0).to_string_lossy();
            let descriptor = MpqShellProvider::load_descriptor_from_path(&path)?;
            self.set_state_from_descriptor(Some(path.clone()), None, descriptor);
            log(format!("IInitializeWithItem: path={}", path));
        }
        Ok(())
    }
}

impl IInitializeWithFile_Impl for MpqShellProvider_Impl {
    #[allow(non_snake_case)]
    fn Initialize(&self, psz_file_path: &PCWSTR, _grf_mode: u32) -> wcore::Result<()> {
        if psz_file_path.is_null() || psz_file_path.0.is_null() {
            return Err(Error::from(E_POINTER));
        }
        let path = unsafe { widestring::U16CStr::from_ptr_str(psz_file_path.0).to_string_lossy() };
        let descriptor = MpqShellProvider::load_descriptor_from_path(&path)?;
        self.set_state_from_descriptor(Some(path.clone()), None, descriptor);
        log(format!("IInitializeWithFile: path={}", path));
        Ok(())
    }
}

impl IInitializeWithStream_Impl for MpqShellProvider_Impl {
    #[allow(non_snake_case)]
    fn Initialize(&self, pstream: windows::core::Ref<'_, IStream>, _grf_mode: u32) -> wcore::Result<()> {
        log("IInitializeWithStream: begin");
        let stream: &IStream = pstream.ok()?;
        unsafe {
            stream.Seek(0, STREAM_SEEK_SET, None)?;
        }

        let seq: ISequentialStream = stream.cast()?;
        let mut buffer = Vec::<u8>::new();
        let mut chunk = [0u8; 8192];

        loop {
            let mut read = 0u32;
            let hr = unsafe { seq.Read(chunk.as_mut_ptr() as *mut _, chunk.len() as u32, Some(&mut read as *mut u32)) };

            if hr.is_err() {
                log(format!("IInitializeWithStream: Read failed hr=0x{:08X}", hr.0 as u32));
                return Err(Error::from(hr));
            }

            if read == 0 {
                break;
            }

            buffer.extend_from_slice(&chunk[..read as usize]);
        }

        if buffer.is_empty() {
            log("IInitializeWithStream: stream empty");
            return Err(Error::from(E_FAIL));
        }

        let bytes: Arc<[u8]> = Arc::from(buffer.into_boxed_slice());
        let descriptor = MpqShellProvider::load_descriptor_from_bytes(&bytes)?;
        self.set_state_from_descriptor(None, Some(bytes), descriptor);
        log("IInitializeWithStream: cached stream buffer");
        Ok(())
    }
}

// ============================
// IThumbnailProvider stub
// ============================

impl IThumbnailProvider_Impl for MpqShellProvider_Impl {
    #[allow(non_snake_case)]
    fn GetThumbnail(&self, _cx: u32, phbmp: *mut HBITMAP, pdwalpha: *mut WTS_ALPHATYPE) -> WinResult<()> {
        if phbmp.is_null() || pdwalpha.is_null() {
            return Err(Error::from(E_POINTER));
        }

        log("GetThumbnail: stub invoked (not implemented)");
        unsafe {
            *phbmp = HBITMAP(ptr::null_mut());
            *pdwalpha = WTS_ALPHATYPE(0);
        }
        Err(Error::from(E_NOTIMPL))
    }
}

// ============================
// IPersistFile implementation
// ============================

#[allow(non_snake_case)]
impl windows::Win32::System::Com::IPersist_Impl for MpqShellProvider_Impl {
    fn GetClassID(&self) -> wcore::Result<windows_core::GUID> {
        log("IPersist::GetClassID");
        Ok(CLSID_MPQ_FOLDER)
    }
}

#[allow(non_snake_case)]
impl IPersistFile_Impl for MpqShellProvider_Impl {
    fn IsDirty(&self) -> windows_core::HRESULT {
        log("IPersistFile::IsDirty -> S_FALSE");
        S_FALSE
    }

    fn Load(&self, pszfilename: &PCWSTR, dwmode: STGM) -> wcore::Result<()> {
        if pszfilename.is_null() || pszfilename.0.is_null() {
            return Err(Error::from(E_POINTER));
        }
        let path = unsafe { widestring::U16CStr::from_ptr_str(pszfilename.0).to_string_lossy() };
        log(format!("IPersistFile::Load path={} mode=0x{:X}", path, dwmode.0));
        let descriptor = MpqShellProvider::load_descriptor_from_path(&path)?;
        self.set_state_from_descriptor(Some(path), None, descriptor);
        Ok(())
    }

    fn Save(&self, _pszfilename: &PCWSTR, _fremember: windows_core::BOOL) -> wcore::Result<()> {
        log("IPersistFile::Save -> STG_E_ACCESSDENIED");
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn SaveCompleted(&self, _pszfilename: &PCWSTR) -> wcore::Result<()> {
        log("IPersistFile::SaveCompleted (noop)");
        Ok(())
    }

    fn GetCurFile(&self) -> wcore::Result<PWSTR> {
        let path_opt = {
            let st = self.state.lock().unwrap();
            st.path_utf8.clone()
        };
        match path_opt {
            Some(path) => {
                log(format!("IPersistFile::GetCurFile returning {}", path));
                allocate_com_string(&path)
            }
            None => {
                log("IPersistFile::GetCurFile no path available");
                Err(Error::from(STG_E_FILENOTFOUND))
            }
        }
    }
}

// ============================
// IStorage skeleton
// ============================

#[allow(non_snake_case)]
impl IStorage_Impl for MpqShellProvider_Impl {
    fn CreateStream(&self, pwcsname: &PCWSTR, grfmode: STGM, _reserved1: u32, _reserved2: u32) -> wcore::Result<IStream> {
        let name = Self::name_from_pwcs(pwcsname);
        log(format!("IStorage::CreateStream stub name={} mode=0x{:X}", name, grfmode.0));
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn OpenStream(&self, pwcsname: &PCWSTR, _reserved1: *const c_void, grfmode: STGM, _reserved2: u32) -> wcore::Result<IStream> {
        let name = Self::name_from_pwcs(pwcsname);
        let descriptor = self.descriptor();
        log(format!("IStorage::OpenStream request name='{}' mode=0x{:X}", name, grfmode.0));

        if name.is_empty() {
            log("IStorage::OpenStream: empty name requested");
            return Err(Error::from(E_FAIL));
        }

        let entry = descriptor
            .find_entry(&name)
            .or_else(|| descriptor.find_entry(&format!("./{}", name)))
            .ok_or_else(|| {
                log(format!("IStorage::OpenStream: entry '{}' not found", name));
                Error::from(STG_E_FILENOTFOUND)
            })?;

        unsafe {
            match SHCreateMemStream(Some(entry.data.as_ref())) {
                Some(stream) => {
                    log(format!("IStorage::OpenStream: returning in-memory stream ({} bytes)", entry.uncompressed_size));
                    Ok(stream)
                }
                None => {
                    log("IStorage::OpenStream: SHCreateMemStream returned NULL");
                    Err(Error::from(E_FAIL))
                }
            }
        }
    }

    fn CreateStorage(&self, pwcsname: &PCWSTR, grfmode: STGM, _reserved1: u32, _reserved2: u32) -> wcore::Result<windows::Win32::System::Com::StructuredStorage::IStorage> {
        let name = Self::name_from_pwcs(pwcsname);
        log(format!("IStorage::CreateStorage stub name={} mode=0x{:X}", name, grfmode.0));
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn OpenStorage(&self, pwcsname: &PCWSTR, _pstgpriority: windows::core::Ref<'_, windows::Win32::System::Com::StructuredStorage::IStorage>, grfmode: STGM, _snbexclude: *const *const u16, _reserved: u32) -> wcore::Result<windows::Win32::System::Com::StructuredStorage::IStorage> {
        let name = Self::name_from_pwcs(pwcsname);
        log(format!("IStorage::OpenStorage stub name={} mode=0x{:X}", name, grfmode.0));
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn CopyTo(&self, _ciidexclude: u32, _rgiidexclude: *const windows_core::GUID, _snbexclude: *const *const u16, _pstgdest: windows::core::Ref<'_, windows::Win32::System::Com::StructuredStorage::IStorage>) -> wcore::Result<()> {
        log("IStorage::CopyTo: read-only placeholder");
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn MoveElementTo(&self, pwcsname: &PCWSTR, _pstgdest: windows::core::Ref<'_, windows::Win32::System::Com::StructuredStorage::IStorage>, pwcsnewname: &PCWSTR, _grfflags: &STGMOVE) -> wcore::Result<()> {
        let old_name = Self::name_from_pwcs(pwcsname);
        let new_name = Self::name_from_pwcs(pwcsnewname);
        log(format!("IStorage::MoveElementTo stub name={} new_name={}", old_name, new_name));
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn Commit(&self, _grfcommitflags: u32) -> wcore::Result<()> {
        log("IStorage::Commit called (no-op for read-only placeholder)");
        Ok(())
    }

    fn Revert(&self) -> wcore::Result<()> {
        log("IStorage::Revert called (no-op for read-only placeholder)");
        Ok(())
    }

    fn EnumElements(&self, _reserved1: u32, _reserved2: *const c_void, _reserved3: u32) -> wcore::Result<IEnumSTATSTG> {
        let descriptor = self.descriptor();
        let count = descriptor.entries().len();
        log(format!("IStorage::EnumElements: enumerating {} entries", count));
        let enumerator: IEnumSTATSTG = MpqEnumStatStg::new(descriptor, 0).into();
        Ok(enumerator)
    }

    fn DestroyElement(&self, pwcsname: &PCWSTR) -> wcore::Result<()> {
        let name = Self::name_from_pwcs(pwcsname);
        log(format!("IStorage::DestroyElement stub name={}", name));
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn RenameElement(&self, pwcsoldname: &PCWSTR, pwcsnewname: &PCWSTR) -> wcore::Result<()> {
        let old_name = Self::name_from_pwcs(pwcsoldname);
        let new_name = Self::name_from_pwcs(pwcsnewname);
        log(format!("IStorage::RenameElement stub old={} new={}", old_name, new_name));
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn SetElementTimes(&self, pwcsname: &PCWSTR, _pctime: *const windows::Win32::Foundation::FILETIME, _patime: *const windows::Win32::Foundation::FILETIME, _pmtime: *const windows::Win32::Foundation::FILETIME) -> wcore::Result<()> {
        let name = Self::name_from_pwcs(pwcsname);
        log(format!("IStorage::SetElementTimes stub name={}", name));
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn SetClass(&self, clsid: *const windows_core::GUID) -> wcore::Result<()> {
        let clsid_string = unsafe { if clsid.is_null() { "NULL".to_string() } else { (&*clsid).to_braced_upper() } };
        log(format!("IStorage::SetClass stub clsid={}", clsid_string));
        Ok(())
    }

    fn SetStateBits(&self, _grfstatebits: u32, _grfmask: u32) -> wcore::Result<()> {
        log("IStorage::SetStateBits stub");
        Ok(())
    }

    fn Stat(&self, pstatstg: *mut STATSTG, _grfstatflag: u32) -> wcore::Result<()> {
        if pstatstg.is_null() {
            return Err(Error::from(E_POINTER));
        }
        let descriptor = self.descriptor();
        let mut stat = STATSTG::default();

        let name_required = (_grfstatflag & STATFLAG_NONAME.0 as u32) == 0;
        if name_required {
            let friendly = self.friendly_storage_name();
            stat.pwcsName = allocate_com_string(&friendly)?;
        }

        stat.r#type = STGTY_STORAGE.0 as u32;
        stat.cbSize = descriptor.total_uncompressed_size();
        stat.grfMode = STGM_READ;
        stat.clsid = CLSID_MPQ_FOLDER;
        unsafe {
            *pstatstg = stat;
        }
        Ok(())
    }
}

#[implement(windows::Win32::System::Com::StructuredStorage::IEnumSTATSTG)]
struct MpqEnumStatStg {
    descriptor: Arc<MpqArchiveDescriptor>,
    position: Mutex<usize>,
}

impl MpqEnumStatStg {
    fn new(descriptor: Arc<MpqArchiveDescriptor>, position: usize) -> Self {
        Self { descriptor, position: Mutex::new(position) }
    }

    fn stat_for_entry(entry: &MpqEntry) -> wcore::Result<STATSTG> {
        let mut stat = STATSTG::default();
        stat.pwcsName = allocate_com_string(&entry.path)?;
        stat.r#type = STGTY_STREAM.0 as u32;
        stat.cbSize = entry.uncompressed_size;
        stat.grfMode = STGM_READ;
        Ok(stat)
    }
}

#[allow(non_snake_case)]
impl IEnumSTATSTG_Impl for MpqEnumStatStg_Impl {
    fn Next(&self, celt: u32, rgelt: *mut STATSTG, pceltfetched: *mut u32) -> wcore::Result<()> {
        if rgelt.is_null() {
            return Err(Error::from(E_POINTER));
        }
        if celt > 1 && pceltfetched.is_null() {
            return Err(Error::from(E_POINTER));
        }

        let mut written = 0u32;
        let entries = self.descriptor.entries();
        let mut pos = self.position.lock().unwrap();

        while written < celt && *pos < entries.len() {
            let stat = MpqEnumStatStg::stat_for_entry(&entries[*pos])?;
            unsafe {
                *rgelt.add(written as usize) = stat;
            }
            *pos += 1;
            written += 1;
        }

        if !pceltfetched.is_null() {
            unsafe {
                *pceltfetched = written;
            }
        }

        if written == celt { Ok(()) } else { Err(Error::from(S_FALSE)) }
    }

    fn Skip(&self, celt: u32) -> wcore::Result<()> {
        let entries_len = self.descriptor.entries().len();
        let mut pos = self.position.lock().unwrap();
        let target = (*pos).saturating_add(celt as usize);
        if target > entries_len {
            *pos = entries_len;
            Err(Error::from(S_FALSE))
        } else {
            *pos = target;
            Ok(())
        }
    }

    fn Reset(&self) -> wcore::Result<()> {
        let mut pos = self.position.lock().unwrap();
        *pos = 0;
        Ok(())
    }

    fn Clone(&self) -> wcore::Result<IEnumSTATSTG> {
        let pos = *self.position.lock().unwrap();
        Ok(MpqEnumStatStg::new(self.descriptor.clone(), pos).into())
    }
}

fn allocate_com_string(text: &str) -> wcore::Result<PWSTR> {
    let wide = widestring::U16CString::from_str(text).map_err(|_| Error::from(E_FAIL))?;
    let slice = wide.as_slice_with_nul();
    let byte_len = slice.len() * size_of::<u16>();
    unsafe {
        let mem = CoTaskMemAlloc(byte_len) as *mut u16;
        if mem.is_null() {
            return Err(Error::from(E_OUTOFMEMORY));
        }
        ptr::copy_nonoverlapping(slice.as_ptr(), mem, slice.len());
        Ok(PWSTR(mem))
    }
}
