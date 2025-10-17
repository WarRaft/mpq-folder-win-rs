use crate::DLL_LOCK_COUNT;
use crate::log::log;
use crate::mpq_shell_provider::MpqShellProvider;
use crate::utils::guid::GuidExt;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::Ordering;
use windows::Win32::System::Com::IClassFactory_Impl;
use windows_core::{BOOL, GUID, IUnknown};
use windows_implement::implement;

#[inline]
fn iid_name(iid: &GUID) -> &'static str {
    use windows::Win32::System::Com::IPersistFile;
    use windows::Win32::System::Com::StructuredStorage::IStorage;
    use windows::Win32::UI::Shell::{IPersistFolder, IShellFolder, IInitializeWithItem, IThumbnailProvider, PropertiesSystem::IInitializeWithFile, PropertiesSystem::IInitializeWithStream};
    use windows_core::Interface;
    if *iid == <IUnknown as Interface>::IID {
        "IUnknown"
    } else if *iid == <IThumbnailProvider as Interface>::IID {
        "IThumbnailProvider"
    } else if *iid == <IInitializeWithItem as Interface>::IID {
        "IInitializeWithItem"
    } else if *iid == <IInitializeWithStream as Interface>::IID {
        "IInitializeWithStream"
    } else if *iid == <IInitializeWithFile as Interface>::IID {
        "IInitializeWithFile"
    } else if *iid == <IStorage as Interface>::IID {
        "IStorage"
    } else if *iid == <IShellFolder as Interface>::IID {
        "IShellFolder"
    } else if *iid == <IPersistFolder as Interface>::IID {
        "IPersistFolder"
    } else if *iid == <IPersistFile as Interface>::IID {
        "IPersistFile"
    } else {
        "UnknownIID"
    }
}

#[inline]
fn ptr_hex<T>(p: *const T) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(18);
    let _ = write!(&mut s, "0x{:016X}", p as usize);
    s
}

#[implement(windows::Win32::System::Com::IClassFactory)]
pub struct MpqClassFactory {
    _placeholder: (),
}

impl MpqClassFactory {
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl IClassFactory_Impl for MpqClassFactory_Impl {
    #[allow(non_snake_case)]
    fn CreateInstance(
        &self,
        _outer: windows::core::Ref<'_, IUnknown>, // aggregation not supported for shell handlers
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        use windows::Win32::Foundation::{E_NOINTERFACE, E_POINTER};
        use windows::Win32::System::Com::IPersistFile;
        use windows::Win32::System::Com::StructuredStorage::IStorage;
        use windows::Win32::UI::Shell::PropertiesSystem::{IInitializeWithFile, IInitializeWithStream};
        use windows::Win32::UI::Shell::{IPersistFolder, IShellFolder, IInitializeWithItem, IThumbnailProvider};
        use windows::core::{Error, IUnknown, Interface};

        // Log call and raw args first
        let riid_log = if riid.is_null() {
            "riid=NULL".to_string()
        } else {
            let gref: &GUID = unsafe { &*riid };
            let name = iid_name(gref);
            let g = gref.to_braced_upper();
            format!("riid={} {}", name, g)
        };
        log(format!("IClassFactory::CreateInstance outer=(aggregation unsupported) {} ppv={}", riid_log, ptr_hex(ppv),));

        if ppv.is_null() || riid.is_null() {
            log("IClassFactory::CreateInstance result=E_POINTER");
            return Err(Error::from(E_POINTER));
        }
        unsafe {
            *ppv = null_mut();
        } // clear out param
        log("IClassFactory::CreateInstance ppv <- NULL");

        // Construct the concrete object
        let unk: IUnknown = {
            log("IClassFactory::CreateInstance new=MpqShellProvider");
            MpqShellProvider::new().into()
        };

        unsafe {
            if *riid == <IThumbnailProvider as Interface>::IID {
                log("IClassFactory::CreateInstance returning IThumbnailProvider");
                *ppv = unk.cast::<IThumbnailProvider>()?.into_raw();
                return Ok(());
            }

            if *riid == <IInitializeWithItem as Interface>::IID {
                log("IClassFactory::CreateInstance returning IInitializeWithItem");
                *ppv = unk.cast::<IInitializeWithItem>()?.into_raw();
                return Ok(());
            }
            if *riid == <IInitializeWithStream as Interface>::IID {
                log("IClassFactory::CreateInstance returning IInitializeWithStream");
                *ppv = unk.cast::<IInitializeWithStream>()?.into_raw();
                return Ok(());
            }
            if *riid == <IInitializeWithFile as Interface>::IID {
                log("IClassFactory::CreateInstance returning IInitializeWithFile");
                *ppv = unk.cast::<IInitializeWithFile>()?.into_raw();
                return Ok(());
            }
            if *riid == <IStorage as Interface>::IID {
                log("IClassFactory::CreateInstance returning IStorage");
                *ppv = unk.cast::<IStorage>()?.into_raw();
                return Ok(());
            }
            if *riid == <IShellFolder as Interface>::IID {
                log("IClassFactory::CreateInstance returning IShellFolder");
                let res = unk.cast::<IShellFolder>();
                match res {
                    Ok(ptr) => {
                        *ppv = ptr.into_raw();
                        log("IClassFactory::CreateInstance: IShellFolder cast OK");
                        return Ok(());
                    }
                    Err(e) => {
                        log(format!("IClassFactory::CreateInstance: IShellFolder cast ERR: {:?}", e));
                        return Err(e);
                    }
                }
            }
            // Handle IShellFolder2 / Namespace Extension GUID {00021500-0000-0000-C000-000000000046}
            if *riid == GUID::from_u128(0x00021500_0000_0000_C000_000000000046) {
                log("IClassFactory::CreateInstance returning IShellFolder for namespace extension GUID");
                let res = unk.cast::<IShellFolder>();
                match res {
                    Ok(ptr) => {
                        *ppv = ptr.into_raw();
                        log("IClassFactory::CreateInstance: IShellFolder2 cast OK");
                        return Ok(());
                    }
                    Err(e) => {
                        log(format!("IClassFactory::CreateInstance: IShellFolder2 cast ERR: {:?}", e));
                        return Err(e);
                    }
                }
            }
            if *riid == <IPersistFolder as Interface>::IID {
                log("IClassFactory::CreateInstance returning IPersistFolder");
                *ppv = unk.cast::<IPersistFolder>()?.into_raw();
                return Ok(());
            }
            if *riid == <IPersistFile as Interface>::IID {
                log("IClassFactory::CreateInstance returning IPersistFile");
                *ppv = unk.cast::<IPersistFile>()?.into_raw();
                return Ok(());
            }
            if *riid == <IUnknown as Interface>::IID {
                log("IClassFactory::CreateInstance returning IUnknown");
                *ppv = unk.into_raw();
                return Ok(());
            }
        }

        log("IClassFactory::CreateInstance result=E_NOINTERFACE");
        Err(Error::from(E_NOINTERFACE))
    }

    #[allow(non_snake_case)]
    fn LockServer(&self, f_lock: BOOL) -> windows::core::Result<()> {
        if f_lock.as_bool() {
            let new = DLL_LOCK_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
            log(format!("IClassFactory::LockServer lock=true new_lock_count={}", new));
        } else {
            let new = DLL_LOCK_COUNT.fetch_sub(1, Ordering::SeqCst) - 1;
            log(format!("IClassFactory::LockServer lock=false new_lock_count={}", new));
        }
        Ok(())
    }
}
