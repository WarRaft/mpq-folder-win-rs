use crate::{CLSID_MPQ_FOLDER, DLL_LOCK_COUNT, ProviderState, archive::MpqArchiveDescriptor, archive::MpqEntry};
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use windows_implement::implement;

use crate::archive::MpqArchiveError;
use crate::log::log;
use crate::utils::guid::GuidExt;
use windows::{
    Win32::{
        Foundation::{E_FAIL, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER, HWND, S_FALSE, STG_E_ACCESSDENIED, STG_E_FILENOTFOUND},
        Graphics::Gdi::HBITMAP,
        System::Com::{
            CoTaskMemAlloc,
            IBindCtx, //
            IPersistFile,
            IPersistFile_Impl,
            ISequentialStream,
            IStream,
            STATFLAG_NONAME,
            STATSTG,
            STGM,
            STGM_READ,
            STGTY_STORAGE,
            STGTY_STREAM,
            STREAM_SEEK_SET,
            StructuredStorage::IStorage,
            StructuredStorage::{IEnumSTATSTG, IEnumSTATSTG_Impl, IStorage_Impl, STGMOVE},
        },
        UI::Shell::PropertiesSystem::IInitializeWithFile,
        UI::Shell::{
            Common::{ITEMIDLIST, STRRET},
            IInitializeWithItem, //
            IInitializeWithItem_Impl,
            ILGetSize,
            IPersistFolder_Impl,
            IShellFolder_Impl,
            IShellFolder2,
            IShellItem,
            IThumbnailProvider_Impl,
            PropertiesSystem::{
                IInitializeWithFile_Impl, //
                IInitializeWithStream,
                IInitializeWithStream_Impl,
            },
            SHCreateMemStream,
            SHGDNF,
            SIGDN_FILESYSPATH,
            WTS_ALPHATYPE,
        },
        UI::Shell::{IPersistFolder, IShellFolder},
    },
    core::{self as wcore, Error, Interface, PCWSTR, PWSTR, Result as WinResult},
};

#[implement(IThumbnailProvider, IInitializeWithItem, IInitializeWithStream, IInitializeWithFile, IStorage, IPersistFile, IPersistFolder, IShellFolder, IShellFolder2)]
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

    fn set_root_pidl_bytes(&self, data: Option<Vec<u8>>) {
        let mut st = self.state.lock().unwrap();
        st.root_pidl = data;
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

#[allow(non_snake_case)]
impl IPersistFolder_Impl for MpqShellProvider_Impl {
    fn Initialize(&self, pidl: *const ITEMIDLIST) -> wcore::Result<()> {
        log(format!("IPersistFolder::Initialize pidl={:p}", pidl));
        let clone = clone_pidl(pidl);
        self.set_root_pidl_bytes(clone);
        Ok(())
    }
}

#[allow(non_snake_case)]
impl IShellFolder_Impl for MpqShellProvider_Impl {
    fn ParseDisplayName(&self, _hwnd: HWND, _pbc: windows_core::Ref<IBindCtx>, pszdisplayname: &PCWSTR, pcheaten: *const u32, ppidl: *mut *mut ITEMIDLIST, pdwattributes: *mut u32) -> wcore::Result<()> {
        let name = Self::name_from_pwcs(pszdisplayname);
        log(format!("IShellFolder::ParseDisplayName called for '{}'", name));
        if name.eq_ignore_ascii_case("TEST.txt") {
            unsafe {
                // Allocate a dummy PIDL (one byte)
                let pidl = CoTaskMemAlloc(2) as *mut u8;
                if !pidl.is_null() {
                    *pidl = 1; // simple marker
                    *pidl.add(1) = 0; // null terminator
                    if !ppidl.is_null() {
                        *ppidl = pidl as *mut ITEMIDLIST;
                    }
                    if !pcheaten.is_null() {
                        *(pcheaten as *mut u32) = name.len() as u32;
                    }
                    if !pdwattributes.is_null() {
                        *pdwattributes = 0x00000080; // SFGAO_STREAM
                    }
                    log("IShellFolder::ParseDisplayName: returned dummy PIDL for TEST.txt");
                    return Ok(());
                }
            }
        }
        Err(Error::from(E_FAIL))
    }

    fn EnumObjects(&self, _hwnd: HWND, _grfflags: u32, ppenumidlist: windows_core::OutRef<windows::Win32::UI::Shell::IEnumIDList>) -> windows_core::HRESULT {
        log("IShellFolder::EnumObjects: returning one TEST.txt item");
        #[implement(windows::Win32::UI::Shell::IEnumIDList)]
        pub struct EnumOneItemImpl {
            pub fetched: std::cell::Cell<bool>,
        }
        impl windows::Win32::UI::Shell::IEnumIDList_Impl for EnumOneItemImpl_Impl {
            fn Next(&self, celt: u32, rgelt: *mut *mut ITEMIDLIST, pceltfetched: *mut u32) -> windows::core::HRESULT {
                unsafe {
                    if self.fetched.get() || celt == 0 || rgelt.is_null() {
                        if !pceltfetched.is_null() {
                            *pceltfetched = 0;
                        }
                        return S_FALSE;
                    }
                    let pidl = CoTaskMemAlloc(2) as *mut u8;
                    if !pidl.is_null() {
                        *pidl = 1;
                        *pidl.add(1) = 0;
                        *rgelt = pidl as *mut ITEMIDLIST;
                        if !pceltfetched.is_null() {
                            *pceltfetched = 1;
                        }
                        self.fetched.set(true);
                        return windows::Win32::Foundation::S_OK;
                    }
                    E_FAIL
                }
            }
            fn Skip(&self, _celt: u32) -> windows::core::HRESULT {
                windows::Win32::Foundation::S_OK
            }
            fn Reset(&self) -> windows::core::HRESULT {
                self.fetched.set(false);
                windows::Win32::Foundation::S_OK
            }
            fn Clone(&self, ppenum: windows::core::OutRef<windows::Win32::UI::Shell::IEnumIDList>) -> windows::core::HRESULT {
                let clone = windows::Win32::UI::Shell::IEnumIDList::from(EnumOneItemImpl { fetched: std::cell::Cell::new(false) });
                let _ = ppenum.write(Some(clone));
                windows::Win32::Foundation::S_OK
            }
        }
        let enum_obj = windows::Win32::UI::Shell::IEnumIDList::from(EnumOneItemImpl { fetched: std::cell::Cell::new(false) });
        let _ = ppenumidlist.write(Some(enum_obj));
        windows::Win32::Foundation::S_OK.into()
    }

    fn BindToObject(&self, _pidl: *const ITEMIDLIST, _pbc: windows_core::Ref<IBindCtx>, _riid: *const windows_core::GUID, _ppv: *mut *mut c_void) -> wcore::Result<()> {
        log("IShellFolder::BindToObject: stub (no subfolders)");
        Err(Error::from(E_NOTIMPL))
    }

    fn BindToStorage(&self, _pidl: *const ITEMIDLIST, _pbc: windows_core::Ref<IBindCtx>, _riid: *const windows_core::GUID, _ppv: *mut *mut c_void) -> wcore::Result<()> {
        log("IShellFolder::BindToStorage: stub");
        Err(Error::from(E_NOTIMPL))
    }

    fn CompareIDs(&self, _lparam: windows::Win32::Foundation::LPARAM, _pidl1: *const ITEMIDLIST, _pidl2: *const ITEMIDLIST) -> windows_core::HRESULT {
        log("IShellFolder::CompareIDs: stub");
        S_FALSE.into()
    }

    fn CreateViewObject(&self, _hwndowner: HWND, _riid: *const windows_core::GUID, _ppv: *mut *mut c_void) -> wcore::Result<()> {
        log("IShellFolder::CreateViewObject: stub");
        Err(Error::from(E_NOTIMPL))
    }

    fn GetAttributesOf(&self, _cidl: u32, _apidl: *const *const ITEMIDLIST, rgfinout: *mut u32) -> wcore::Result<()> {
        log("IShellFolder::GetAttributesOf: returning SFGAO_STREAM");
        unsafe {
            if !rgfinout.is_null() {
                *rgfinout = 0x00000080; // SFGAO_STREAM
            }
        }
        Ok(())
    }

    fn GetUIObjectOf(&self, _hwndowner: HWND, _cidl: u32, _apidl: *const *const ITEMIDLIST, _riid: *const windows_core::GUID, _rgfreserved: *const u32, _ppv: *mut *mut c_void) -> wcore::Result<()> {
        log("IShellFolder::GetUIObjectOf: stub");
        Err(Error::from(E_NOTIMPL))
    }

    fn GetDisplayNameOf(&self, _pidl: *const ITEMIDLIST, _uflags: SHGDNF, pname: *mut STRRET) -> wcore::Result<()> {
        log("IShellFolder::GetDisplayNameOf: returning TEST.txt");
        unsafe {
            if !pname.is_null() {
                (*pname).uType = 1; // STRRET_CSTR
                let s = b"TEST.txt\0";
                ptr::copy_nonoverlapping(s.as_ptr(), (*pname).Anonymous.cStr.as_mut_ptr(), s.len());
            }
        }
        Ok(())
    }

    fn SetNameOf(&self, _hwnd: HWND, _pidl: *const ITEMIDLIST, _pszname: &PCWSTR, _uflags: SHGDNF, _ppidlout: *mut *mut ITEMIDLIST) -> wcore::Result<()> {
        log("IShellFolder::SetNameOf: stub");
        Err(Error::from(E_NOTIMPL))
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
        log(format!("IStorage::OpenStorage name='{}' mode=0x{:X}", name, grfmode.0));
        let descriptor = self.descriptor();
        if name.is_empty() {
            return Err(Error::from(E_FAIL));
        }
        let storage: windows::Win32::System::Com::StructuredStorage::IStorage = MpqStorage::new(descriptor, name).into();
        Ok(storage)
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
        let enumerator: IEnumSTATSTG = MpqEnumElements::new(descriptor, String::new()).into();
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

// ============
// Sub-storage
// ============

fn normalize_join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", prefix.trim_end_matches('/'), name.trim_start_matches('/'))
    }
}

fn list_children(descriptor: &MpqArchiveDescriptor, prefix: &str) -> (Vec<String>, Vec<usize>) {
    use std::collections::HashSet;
    let mut dirs: HashSet<String> = HashSet::new();
    let mut files: Vec<usize> = Vec::new();
    let pre = if prefix.is_empty() { String::new() } else { format!("{}/", prefix.trim_end_matches('/')) };
    for (idx, e) in descriptor.entries().iter().enumerate() {
        let p = &e.path;
        if !pre.is_empty() {
            if !p.starts_with(&pre) {
                continue;
            }
        }
        let rel = if pre.is_empty() { p.as_str() } else { &p[pre.len()..] };
        let rel = rel.trim_matches(['/', '\\']);
        if rel.is_empty() {
            continue;
        }
        if let Some(pos) = rel.find(['/', '\\']) {
            let d = &rel[..pos];
            if !d.is_empty() {
                dirs.insert(d.to_string());
            }
        } else {
            files.push(idx);
        }
    }
    let mut dlist: Vec<String> = dirs.into_iter().collect();
    dlist.sort_unstable();
    files.sort_unstable();
    (dlist, files)
}

#[implement(windows::Win32::System::Com::StructuredStorage::IStorage)]
struct MpqStorage {
    descriptor: Arc<MpqArchiveDescriptor>,
    prefix: String,
}

impl MpqStorage {
    fn new(descriptor: Arc<MpqArchiveDescriptor>, prefix: String) -> Self {
        Self { descriptor, prefix }
    }

    fn open_stream_inner(&self, name: &str) -> wcore::Result<IStream> {
        let full1 = normalize_join(&self.prefix, name);
        let full2 = full1.replace('/', "\\");
        let entry = self
            .descriptor
            .find_entry(&full1)
            .or_else(|| self.descriptor.find_entry(&full2))
            .ok_or_else(|| Error::from(STG_E_FILENOTFOUND))?;
        unsafe {
            match SHCreateMemStream(Some(entry.data.as_ref())) {
                Some(s) => Ok(s),
                None => Err(Error::from(E_FAIL)),
            }
        }
    }
}

#[allow(non_snake_case)]
impl IStorage_Impl for MpqStorage_Impl {
    fn CreateStream(&self, _pwcsname: &PCWSTR, _grfmode: STGM, _reserved1: u32, _reserved2: u32) -> wcore::Result<IStream> {
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn OpenStream(&self, pwcsname: &PCWSTR, _reserved1: *const c_void, _grfmode: STGM, _reserved2: u32) -> wcore::Result<IStream> {
        let name = MpqShellProvider_Impl::name_from_pwcs(pwcsname);
        self.open_stream_inner(&name)
    }

    fn CreateStorage(&self, _pwcsname: &PCWSTR, _grfmode: STGM, _reserved1: u32, _reserved2: u32) -> wcore::Result<windows::Win32::System::Com::StructuredStorage::IStorage> {
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn OpenStorage(&self, pwcsname: &PCWSTR, _pstgpriority: windows::core::Ref<'_, windows::Win32::System::Com::StructuredStorage::IStorage>, _grfmode: STGM, _snbexclude: *const *const u16, _reserved: u32) -> wcore::Result<windows::Win32::System::Com::StructuredStorage::IStorage> {
        let name = MpqShellProvider_Impl::name_from_pwcs(pwcsname);
        let (dirs, _files) = list_children(&self.descriptor, &self.prefix);
        if !dirs.iter().any(|d| d.eq_ignore_ascii_case(&name)) {
            return Err(Error::from(STG_E_FILENOTFOUND));
        }
        let new_prefix = normalize_join(&self.prefix, &name);
        Ok(MpqStorage::new(self.descriptor.clone(), new_prefix).into())
    }

    fn CopyTo(&self, _ciidexclude: u32, _rgiidexclude: *const windows_core::GUID, _snbexclude: *const *const u16, _pstgdest: windows::core::Ref<'_, windows::Win32::System::Com::StructuredStorage::IStorage>) -> wcore::Result<()> {
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn MoveElementTo(&self, _pwcsname: &PCWSTR, _pstgdest: windows::core::Ref<'_, windows::Win32::System::Com::StructuredStorage::IStorage>, _pwcsnewname: &PCWSTR, _grfflags: &STGMOVE) -> wcore::Result<()> {
        Err(Error::from(STG_E_ACCESSDENIED))
    }

    fn Commit(&self, _grfcommitflags: u32) -> wcore::Result<()> {
        Ok(())
    }
    fn Revert(&self) -> wcore::Result<()> {
        Ok(())
    }

    fn EnumElements(&self, _reserved1: u32, _reserved2: *const c_void, _reserved3: u32) -> wcore::Result<IEnumSTATSTG> {
        Ok(MpqEnumElements::new(self.descriptor.clone(), self.prefix.clone()).into())
    }

    fn DestroyElement(&self, _pwcsname: &PCWSTR) -> wcore::Result<()> {
        Err(Error::from(STG_E_ACCESSDENIED))
    }
    fn RenameElement(&self, _pwcsoldname: &PCWSTR, _pwcsnewname: &PCWSTR) -> wcore::Result<()> {
        Err(Error::from(STG_E_ACCESSDENIED))
    }
    fn SetElementTimes(&self, _pwcsname: &PCWSTR, _pctime: *const windows::Win32::Foundation::FILETIME, _patime: *const windows::Win32::Foundation::FILETIME, _pmtime: *const windows::Win32::Foundation::FILETIME) -> wcore::Result<()> {
        Err(Error::from(STG_E_ACCESSDENIED))
    }
    fn SetClass(&self, _clsid: *const windows_core::GUID) -> wcore::Result<()> {
        Ok(())
    }
    fn SetStateBits(&self, _grfstatebits: u32, _grfmask: u32) -> wcore::Result<()> {
        Ok(())
    }

    fn Stat(&self, pstatstg: *mut STATSTG, _grfstatflag: u32) -> wcore::Result<()> {
        if pstatstg.is_null() {
            return Err(Error::from(E_POINTER));
        }
        let mut stat = STATSTG::default();
        let name_required = (_grfstatflag & STATFLAG_NONAME.0 as u32) == 0;
        if name_required {
            stat.pwcsName = allocate_com_string(&self.prefix)?;
        }
        let pre = if self.prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", self.prefix.trim_end_matches('/'))
        };
        let mut total: u64 = 0;
        for e in self.descriptor.entries().iter() {
            if pre.is_empty() || e.path.starts_with(&pre) {
                total = total.saturating_add(e.uncompressed_size);
            }
        }
        stat.r#type = STGTY_STORAGE.0 as u32;
        stat.cbSize = total;
        stat.grfMode = STGM_READ;
        unsafe {
            *pstatstg = stat;
        }
        Ok(())
    }
}

#[derive(Clone)]
enum EnumItem {
    Dir(String),
    File(usize), // index into descriptor.entries()
}

#[implement(windows::Win32::System::Com::StructuredStorage::IEnumSTATSTG)]
struct MpqEnumElements {
    descriptor: Arc<MpqArchiveDescriptor>,
    prefix: String,
    items: Arc<[EnumItem]>,
    position: Mutex<usize>,
}

impl MpqEnumElements {
    fn new(descriptor: Arc<MpqArchiveDescriptor>, prefix: String) -> Self {
        let (dirs, files_idx) = list_children(&descriptor, &prefix);
        let mut items: Vec<EnumItem> = Vec::with_capacity(dirs.len() + files_idx.len());
        for d in dirs {
            items.push(EnumItem::Dir(d));
        }
        for i in files_idx {
            items.push(EnumItem::File(i));
        }
        Self { descriptor, prefix, items: Arc::from(items.into_boxed_slice()), position: Mutex::new(0) }
    }

    fn stat_for_dir(name: &str) -> wcore::Result<STATSTG> {
        let mut stat = STATSTG::default();
        stat.pwcsName = allocate_com_string(name)?;
        stat.r#type = STGTY_STORAGE.0 as u32;
        stat.cbSize = 0;
        stat.grfMode = STGM_READ;
        Ok(stat)
    }

    fn stat_for_file(entry: &MpqEntry) -> wcore::Result<STATSTG> {
        let filename = entry
            .path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(entry.path.as_str())
            .to_string();
        let mut stat = STATSTG::default();
        stat.pwcsName = allocate_com_string(&filename)?;
        stat.r#type = STGTY_STREAM.0 as u32;
        stat.cbSize = entry.uncompressed_size;
        stat.grfMode = STGM_READ;
        Ok(stat)
    }
}

#[allow(non_snake_case)]
impl IEnumSTATSTG_Impl for MpqEnumElements_Impl {
    fn Next(&self, celt: u32, rgelt: *mut STATSTG, pceltfetched: *mut u32) -> wcore::Result<()> {
        if rgelt.is_null() {
            return Err(Error::from(E_POINTER));
        }
        if celt > 1 && pceltfetched.is_null() {
            return Err(Error::from(E_POINTER));
        }

        let mut written = 0u32;
        let mut pos = self.position.lock().unwrap();
        while written < celt && *pos < self.items.len() {
            let stat = match &self.items[*pos] {
                EnumItem::Dir(name) => MpqEnumElements::stat_for_dir(name)?,
                EnumItem::File(idx) => MpqEnumElements::stat_for_file(&self.descriptor.entries()[*idx])?,
            };
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
        let mut pos = self.position.lock().unwrap();
        let target = (*pos).saturating_add(celt as usize);
        if target > self.items.len() {
            *pos = self.items.len();
            Err(Error::from(S_FALSE))
        } else {
            *pos = target;
            Ok(())
        }
    }

    fn Reset(&self) -> wcore::Result<()> {
        *self.position.lock().unwrap() = 0;
        Ok(())
    }

    fn Clone(&self) -> wcore::Result<IEnumSTATSTG> {
        Ok(MpqEnumElements { descriptor: self.descriptor.clone(), prefix: self.prefix.clone(), items: self.items.clone(), position: Mutex::new(*self.position.lock().unwrap()) }.into())
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

fn clone_pidl(pidl: *const ITEMIDLIST) -> Option<Vec<u8>> {
    if pidl.is_null() {
        return None;
    }
    let size = unsafe { ILGetSize(Some(pidl)) } as usize;
    if size == 0 {
        return None;
    }
    let mut buffer = vec![0u8; size];
    unsafe {
        ptr::copy_nonoverlapping(pidl as *const u8, buffer.as_mut_ptr(), size);
    }
    Some(buffer)
}
