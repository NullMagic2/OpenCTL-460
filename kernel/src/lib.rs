//! KMDF virtual-HID pen transport. Pressure math remains in user mode.
#![no_std]
extern crate wdk_panic;

use core::{
    ffi::c_void,
    mem::size_of,
    ptr::{null, null_mut},
};
use wdk_sys::{call_unsafe_wdf_function_binding as wdf, *};
use wdk_sys::{hid::*, ntddk::KeQueryUnbiasedInterruptTime};
#[path = "../../shared/wire.rs"]
mod wire;

#[repr(C)]
struct Context {
    vhf: VHFHANDLE,
    timer: WDFTIMER,
    lock: WDFSPINLOCK,
    last_time: u64,
    last: [u8; wire::REPORT_LEN],
}
struct TypeInfo(WDF_OBJECT_CONTEXT_TYPE_INFO);
// SAFETY: The context descriptor is immutable; its pointers refer only to static data.
unsafe impl Sync for TypeInfo {}
static CONTEXT_TYPE: TypeInfo = TypeInfo(WDF_OBJECT_CONTEXT_TYPE_INFO {
    Size: size_of::<WDF_OBJECT_CONTEXT_TYPE_INFO>() as u32,
    ContextName: b"CTL460Context\0".as_ptr().cast(),
    ContextSize: size_of::<Context>(),
    UniqueType: null(),
    EvtDriverGetUniqueContextType: None,
});

unsafe fn context(device: WDFDEVICE) -> *mut Context {
    // SAFETY: Only devices created below are passed, with this exact context descriptor.
    unsafe {
        wdf!(
            WdfObjectGetTypedContextWorker,
            device.cast(),
            &CONTEXT_TYPE.0
        )
        .cast()
    }
}
fn unicode(text: &mut [u16]) -> UNICODE_STRING {
    UNICODE_STRING {
        Length: ((text.len() - 1) * 2) as u16,
        MaximumLength: (text.len() * 2) as u16,
        Buffer: text.as_mut_ptr(),
    }
}

// SAFETY: Sole driver entry export; Windows supplies valid driver and registry pointers.
#[no_mangle]
pub unsafe extern "system" fn DriverEntry(
    driver: PDRIVER_OBJECT,
    path: PCUNICODE_STRING,
) -> NTSTATUS {
    let mut config = WDF_DRIVER_CONFIG {
        Size: size_of::<WDF_DRIVER_CONFIG>() as u32,
        EvtDriverDeviceAdd: Some(add_device),
        ..Default::default()
    };
    unsafe {
        wdf!(
            WdfDriverCreate,
            driver,
            path,
            null_mut(),
            &mut config,
            null_mut()
        )
    }
}

unsafe extern "C" fn add_device(_: WDFDRIVER, mut init: PWDFDEVICE_INIT) -> NTSTATUS {
    // SAFETY: WDF owns init; every failing initialization returns NTSTATUS to framework cleanup.
    unsafe {
        wdf!(WdfDeviceInitSetDeviceType, init, FILE_DEVICE_UNKNOWN);
        wdf!(
            WdfDeviceInitSetCharacteristics,
            init,
            FILE_DEVICE_SECURE_OPEN,
            1
        );
        wdf!(WdfDeviceInitSetExclusive, init, 1);
        let mut name_buf: [u16; 25] = [
            92, 68, 101, 118, 105, 99, 101, 92, 67, 84, 76, 52, 54, 48, 86, 105, 114, 116, 117, 97,
            108, 80, 101, 110, 0,
        ];
        let status = wdf!(WdfDeviceInitAssignName, init, &unicode(&mut name_buf));
        if status < 0 {
            return status;
        }
        // Only administrators and SYSTEM may submit virtual pen reports in this developer build.
        let mut sddl_buf: [u16; 27] = [
            68, 58, 80, 40, 65, 59, 59, 71, 65, 59, 59, 59, 83, 89, 41, 40, 65, 59, 59, 71, 65, 59,
            59, 59, 66, 65, 41,
        ];
        let mut sddl = [0u16; 28];
        sddl[..27].copy_from_slice(&sddl_buf);
        sddl_buf.fill(0);
        let status = wdf!(WdfDeviceInitAssignSDDLString, init, &unicode(&mut sddl));
        if status < 0 {
            return status;
        }
        let mut files = WDF_FILEOBJECT_CONFIG {
            Size: size_of::<WDF_FILEOBJECT_CONFIG>() as u32,
            EvtFileCleanup: Some(file_cleanup),
            FileObjectClass: _WDF_FILEOBJECT_CLASS::WdfFileObjectWdfCannotUseFsContexts,
            ..Default::default()
        };
        wdf!(
            WdfDeviceInitSetFileObjectConfig,
            init,
            &mut files,
            null_mut()
        );
        let mut attrs = WDF_OBJECT_ATTRIBUTES {
            Size: size_of::<WDF_OBJECT_ATTRIBUTES>() as u32,
            ContextTypeInfo: &CONTEXT_TYPE.0,
            EvtCleanupCallback: Some(cleanup),
            ExecutionLevel: _WDF_EXECUTION_LEVEL::WdfExecutionLevelPassive,
            SynchronizationScope: _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeNone,
            ..Default::default()
        };
        let mut device = null_mut();
        let status = wdf!(WdfDeviceCreate, &mut init, &mut attrs, &mut device);
        if status < 0 {
            return status;
        }
        let ctx = context(device);
        (*ctx).last[0] = 1;
        let mut child_attrs = WDF_OBJECT_ATTRIBUTES {
            Size: size_of::<WDF_OBJECT_ATTRIBUTES>() as u32,
            ParentObject: device.cast(),
            // Match WDF_OBJECT_ATTRIBUTES_INIT: zero means Invalid, not Inherit.
            // These defaults are required by both spin-lock and timer creation.
            ExecutionLevel: _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
            SynchronizationScope:
                _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
            ..Default::default()
        };
        let status = wdf!(WdfSpinLockCreate, &mut child_attrs, &mut (*ctx).lock);
        if status < 0 {
            return status;
        }
        let mut vhf = VHF_CONFIG {
            Size: size_of::<VHF_CONFIG>() as u32,
            DeviceObject: wdf!(WdfDeviceWdmGetDeviceObject, device),
            ReportDescriptorLength: wire::REPORT_DESCRIPTOR.len() as u16,
            ReportDescriptor: wire::REPORT_DESCRIPTOR.as_ptr() as *mut u8,
            VersionNumber: 1,
            ..Default::default()
        };
        // Vendor/Product zero identify an independent virtual source, never impersonate Wacom hardware.
        let status = VhfCreate(&mut vhf, &mut (*ctx).vhf);
        if status < 0 {
            return status;
        }
        let status = VhfStart((*ctx).vhf);
        if status < 0 {
            return status;
        }
        let mut q = WDF_IO_QUEUE_CONFIG {
            Size: size_of::<WDF_IO_QUEUE_CONFIG>() as u32,
            DispatchType: _WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchSequential,
            DefaultQueue: 1,
            EvtIoDeviceControl: Some(ioctl),
            PowerManaged: _WDF_TRI_STATE::WdfTrue,
            ..Default::default()
        };
        let status = wdf!(WdfIoQueueCreate, device, &mut q, null_mut(), null_mut());
        if status < 0 {
            return status;
        }
        let mut t = WDF_TIMER_CONFIG {
            Size: size_of::<WDF_TIMER_CONFIG>() as u32,
            EvtTimerFunc: Some(watchdog),
            Period: 25,
            AutomaticSerialization: 0,
            ..Default::default()
        };
        child_attrs.ExecutionLevel = _WDF_EXECUTION_LEVEL::WdfExecutionLevelDispatch;
        let status = wdf!(WdfTimerCreate, &mut t, &mut child_attrs, &mut (*ctx).timer);
        if status < 0 {
            return status;
        }
        let _ = wdf!(WdfTimerStart, (*ctx).timer, -250_000i64);
        let mut link: [u16; 29] = [
            92, 68, 111, 115, 68, 101, 118, 105, 99, 101, 115, 92, 67, 84, 76, 52, 54, 48, 86, 105,
            114, 116, 117, 97, 108, 80, 101, 110, 0,
        ];
        wdf!(WdfDeviceCreateSymbolicLink, device, &unicode(&mut link))
    }
}

unsafe fn submit_locked(ctx: *mut Context, report: &mut [u8; wire::REPORT_LEN]) -> NTSTATUS {
    // SAFETY: Caller owns ctx.lock; VHF copies the report with default buffering before returning.
    unsafe {
        if (*ctx).vhf.is_null() {
            return STATUS_DEVICE_NOT_READY;
        }
        let mut packet = HID_XFER_PACKET {
            reportBuffer: report.as_mut_ptr(),
            reportBufferLen: wire::REPORT_LEN as u32,
            reportId: 1,
        };
        let status = VhfReadReportSubmit((*ctx).vhf, &mut packet);
        if status >= 0 {
            (*ctx).last = *report;
            (*ctx).last_time = KeQueryUnbiasedInterruptTime();
        }
        status
    }
}
unsafe fn release_locked(ctx: *mut Context) {
    unsafe {
        let mut report = (*ctx).last;
        report[1] = 0;
        report[6..].fill(0);
        let _ = submit_locked(ctx, &mut report);
    }
}
unsafe extern "C" fn ioctl(
    queue: WDFQUEUE,
    request: WDFREQUEST,
    _: usize,
    input_len: usize,
    code: u32,
) {
    // SAFETY: METHOD_BUFFERED request memory is validated before copying; no user pointers retained.
    unsafe {
        let mut status = STATUS_INVALID_DEVICE_REQUEST;
        if code == wire::IOCTL_SUBMIT && input_len == wire::REPORT_LEN {
            let mut ptr: *mut c_void = null_mut();
            let mut actual = 0usize;
            status = wdf!(
                WdfRequestRetrieveInputBuffer,
                request,
                wire::REPORT_LEN,
                &mut ptr,
                &mut actual
            );
            if status >= 0 {
                let bytes = core::slice::from_raw_parts(ptr.cast::<u8>(), wire::REPORT_LEN);
                if !wire::valid_report(bytes) {
                    status = STATUS_INVALID_PARAMETER;
                } else {
                    let mut report = [0u8; wire::REPORT_LEN];
                    report.copy_from_slice(bytes);
                    let ctx = context(wdf!(WdfIoQueueGetDevice, queue));
                    wdf!(WdfSpinLockAcquire, (*ctx).lock);
                    status = submit_locked(ctx, &mut report);
                    wdf!(WdfSpinLockRelease, (*ctx).lock);
                }
            }
        }
        wdf!(WdfRequestComplete, request, status);
    }
}
unsafe extern "C" fn file_cleanup(file: WDFFILEOBJECT) {
    unsafe {
        let ctx = context(wdf!(WdfFileObjectGetDevice, file));
        if !(*ctx).lock.is_null() {
            wdf!(WdfSpinLockAcquire, (*ctx).lock);
            release_locked(ctx);
            wdf!(WdfSpinLockRelease, (*ctx).lock);
        }
    }
}
unsafe extern "C" fn watchdog(timer: WDFTIMER) {
    // SAFETY: Timer is parented to device; cleanup synchronously stops it before destroying VHF.
    unsafe {
        let ctx = context(wdf!(WdfTimerGetParentObject, timer).cast());
        wdf!(WdfSpinLockAcquire, (*ctx).lock);
        if (*ctx).last[1] & 17 != 0
            && KeQueryUnbiasedInterruptTime().saturating_sub((*ctx).last_time) >= 1_000_000
        {
            release_locked(ctx);
        }
        wdf!(WdfSpinLockRelease, (*ctx).lock);
    }
}
unsafe extern "C" fn cleanup(object: WDFOBJECT) {
    // SAFETY: PASSIVE_LEVEL, no spinlock held during a waiting stop/delete. Handles are nulled once.
    unsafe {
        let ctx = context(object.cast());
        if !(*ctx).timer.is_null() {
            let _ = wdf!(WdfTimerStop, (*ctx).timer, 1);
        }
        if !(*ctx).lock.is_null() {
            wdf!(WdfSpinLockAcquire, (*ctx).lock);
        }
        let handle = (*ctx).vhf;
        (*ctx).vhf = null_mut();
        if !(*ctx).lock.is_null() {
            wdf!(WdfSpinLockRelease, (*ctx).lock);
        }
        if !handle.is_null() {
            VhfDelete(handle, 1);
        }
    }
}
