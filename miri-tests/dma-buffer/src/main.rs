#![feature(new_uninit)]
// #![allow(unaligned_references)]

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct Descriptor {
    address: u32,               // physical address to sound data
    length: u16,                // sample count
    control: u16
}

const BUFFER_SIZE: usize = 1 << 16;
const BUFFER_COUNT: usize = 32;

// For ICH5 max number of buffers is 32. The data should be
// aligned on 8-byte boundaries. Each buffer descriptor is 8
// bytes long and the list can contain a maximum of 32
// entries. Empty buffers are allowed but not first and not
// last. Number of samples must be multiple of channel
// count. Left channel is always assumed to be first and
// right channel is assumed to be the last.
#[repr(C, align(8))]
#[derive(Copy, Clone)]
struct BufferDescriptorListWithBuffers {
    descriptors: [Descriptor; BUFFER_COUNT],
    buffers: [[i16; BUFFER_SIZE]; BUFFER_COUNT]
}

fn init_bdl(mapping: u32, bdl: &mut BufferDescriptorListWithBuffers) {
    let bdl_base = bdl as *mut BufferDescriptorListWithBuffers as *mut u8;
    for (descriptor, buffer) in bdl.descriptors.iter_mut().zip(bdl.buffers.iter()) {
        let buffer_offset = unsafe {
            (buffer.as_ptr() as *const u8)
                .offset_from(bdl_base)
        };
        let descriptor_address = unsafe {
            (mapping as *const u8)
                .offset(buffer_offset)
        };
        descriptor.address = descriptor_address as u32;
        descriptor.length = 0;
        descriptor.control = 0;
    }
}

fn main() {
    let mut bdl = Box::<BufferDescriptorListWithBuffers>::new_uninit();

    // TEST#1
    let mut bdl = unsafe { bdl.assume_init() };

    // TEST#2
    let buffer_offset = unsafe {
        (&bdl.buffers[1] as *const i16 as *const u8)
            .offset_from(&mut *bdl as *mut BufferDescriptorListWithBuffers as *mut u8)
    };
    println!("buffer_offset: {}", buffer_offset);

    // TEST#3 -- TBD: is it sufficient to use bdl as a device-relative address?
    let dma_base = &*bdl as *const BufferDescriptorListWithBuffers;
    init_bdl(dma_base as u32, &mut bdl);
    for desc in bdl.descriptors.iter() {
        println!("addr: {:x}", desc.address);
    }
}
