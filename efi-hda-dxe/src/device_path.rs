use core::mem;
use uefi::prelude::*;
use uefi::proto::device_path::{DevicePath, DeviceType, DeviceSubType, HwDeviceSubType};
use alloc::boxed::Box;
use alloc::boxed::*;

pub const HDA_CODEC_DEVICE_PATH_GUID: uefi::Guid = uefi::Guid::from_values(
    0x21e43f8a,
    0x33ff,
    0x4fa9,
    0x929c,
    [0xdb, 0x29, 0x62, 0x26, 0x0f, 0xb8]
);

#[repr(C, packed)]
pub struct HdaDevicePath {
    pub header: DevicePath,
    pub guid: uefi::Guid,
    pub codec_address: u32
}

#[repr(C, packed)]
pub struct CodecDevicePath {
    pub hda: HdaDevicePath,
    pub end: DevicePath
}

fn get_next_device_path_node_mut(device_path: &mut DevicePath) -> Option<&mut DevicePath> {
    let len = usize::from(u16::from_le_bytes(device_path.length));
    let byte_ptr = device_path as *mut DevicePath as *mut u8;

    if device_path.device_type == DeviceType::End {
        None
    } else {
        unsafe {
            let next = byte_ptr.add(len) as *mut DevicePath;
            Some(&mut *next)
        }
    }
}

fn get_next_device_path_node(device_path: &DevicePath) -> Option<&DevicePath> {
    let len = usize::from(u16::from_le_bytes(device_path.length));
    let byte_ptr = device_path as *const DevicePath as *const u8;

    if device_path.device_type == DeviceType::End {
        None
    } else {
        unsafe {
            let next = byte_ptr.add(len) as *const DevicePath;
            Some(&*next)
        }
    }
}

fn get_device_path_size(device_path: &DevicePath) -> usize {
    let mut total_size = 0;
    let mut device_path = device_path;

    loop {
        let len = usize::from(u16::from_le_bytes(device_path.length));
        let byte_ptr = device_path as *const DevicePath as *const u8;

        total_size += len;
        if device_path.device_type == DeviceType::End {
            break;
        }

        unsafe {
            let next = byte_ptr.add(len) as *const DevicePath;
            device_path = &*next;
        }
    }

    total_size
}

unsafe fn copy_device_path_node(dst: &mut DevicePath, src: &DevicePath) {
    let len = usize::from(u16::from_le_bytes(src.length));

    let dst_byte_ptr = dst as *mut DevicePath as *mut u8;
    let src_byte_ptr = src as *const DevicePath as *const u8;

    dst_byte_ptr.copy_from(src_byte_ptr, len);
}

unsafe fn copy_device_path(dst: &mut DevicePath, src: &DevicePath) {
    let mut dst_path = dst as *mut DevicePath;
    let mut src_path = src;
    loop {
        // SAFETY: Safe as long the the storage has sufficient size
        copy_device_path_node(&mut *dst_path, src_path);
        if let Some(dst_node) = get_next_device_path_node_mut(&mut *dst_path) {
            dst_path = dst_node as *mut DevicePath;
        }
        if let Some(src_node) = get_next_device_path_node(src_path) {
            src_path = src_node;
        } else {
            break;
        }
    }
}

pub fn concat_device_path(lhs: &DevicePath, rhs: &DevicePath) -> uefi::Result<Box<DevicePath>> {

    // Allocate space for second end node aswell.. who cares
    let storage_size = get_device_path_size(lhs) + get_device_path_size(rhs);
    let storage = Box::leak(vec![0; storage_size].into_boxed_slice())
        .as_mut_ptr()
        .cast();

    let mut dst_path = storage;

    unsafe {
        // Copy the first path (dangerous)
        copy_device_path(&mut *dst_path, lhs);

        // Loop through nodes and find the end node (very dangerous)
        while (*dst_path).device_type != DeviceType::End {
            if let Some(next_node) = get_next_device_path_node_mut(&mut *dst_path) {
                dst_path = next_node as *mut DevicePath;
            } else {
                // TBD: reaching this points means that the first path is incorrect or worse
                //      the copy_device_path() is broken
                break;
            }
        }

        // Then copy the second path (pure madness)
        copy_device_path(&mut *dst_path, rhs);
    }

    // Box<DevicePath> is sound because DevicePath is repr(C)
    unsafe {
        Ok(Box::from_raw(storage).into())
    }
}

pub fn make_codec_subpath(codec: u32) -> CodecDevicePath {
    CodecDevicePath {
        hda: HdaDevicePath {
            header: DevicePath {
                device_type: DeviceType::Hardware,
                sub_type: unsafe { mem::transmute(HwDeviceSubType::Vendor) },
                length: u16::to_le_bytes(mem::size_of::<HdaDevicePath>() as u16)
            },
            guid: HDA_CODEC_DEVICE_PATH_GUID,
            codec_address: codec
        },
        end: DevicePath {
            device_type: DeviceType::End,
            sub_type: DeviceSubType::EndEntire,
            length: u16::to_le_bytes(mem::size_of::<DevicePath>() as u16)
        }
    }
}
