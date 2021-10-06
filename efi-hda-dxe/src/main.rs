// NB: HDA specification referenced in this source code is
//     HDA Spec 1.0a, June 17, 2011
// NB. Windows 10 lacks sound output using ich6-intel-hda
//     under qemu, ich9 works though
#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// Because there are too many constants that we won't gonna use
#![allow(dead_code)]

// Because extra parens lead to better readability
#![allow(unused_parens)]

// For IoBase
#![feature(const_fn_trait_bound)]

// We are accessing packed structures. Make sure that we
// don't produce undefined behavior
#![deny(unaligned_references)]

// TBD: needed to allocate BDL on the heap without creating
//      it first on the stack and then allocate a heap
//      structure. Can we do better?
#![feature(new_uninit)]

// TBD: move into another crate (for static assert)
#![feature(const_panic)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate uefi;
#[macro_use]
extern crate alloc;
#[macro_use]
extern crate bitflags;
extern crate efi_pcm;
extern crate efi_dxe;

use bitflags::bitflags;
use uefi::prelude::*;
use uefi::proto::device_path::DevicePath;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::driver_binding::DriverBinding;
use uefi::proto::pci::PciIO;
use uefi::table::boot::OpenAttribute;
use uefi::table::boot::BootServices;

use core::str;
use core::fmt::*;
use core::mem;
use alloc::boxed::*;

use efi_dxe::*;
use efi_pcm::*;

mod iobase;
use iobase::*;

// TBD: move into another crate
macro_rules! static_assert {
    ($cond:expr) => {
        static_assert!($cond, concat!("assertion failed: ", stringify!($cond)));
    };
    ($cond:expr, $($t:tt)+) => {
        #[forbid(const_err)]
        const _: () = {
            if !$cond {
                core::panic!($($t)+)
            }
        };
    };
}

// Timeout in microseconds
const INFINITY: u64 = !0;

/// PCI Configuration Space
/// Section 1.1, Intel I/O Controller Hub 7 Family External Design Specification, April 2005
const PCI_VID: u32      = 0x0;                      // ro, u16
const PCI_DID: u32      = 0x2;                      // ro, u16
const PCI_COMMAND: u32  = 0x4;                      // rw, u16
const PCI_STATUS: u32   = 0x6;                      // rwc, u16
const PCI_RID: u32      = 0x8;                      // ro, u8
const PCI_PI: u32       = 0x9;                      // ro, u8
const PCI_SCC: u32      = 0xa;                      // ro, u8
const PCI_BCC: u32      = 0xb;                      // ro, u8
const PCI_CLS: u32      = 0xc;                      // rw, u8
const PCI_LT: u32       = 0xd;                      // ro, u8
const PCI_HEADTYP: u32  = 0xe;                      // ro, u8
const PCI_HDRBARL: u32  = 0x10;                     // rw, u32
const PCI_HDRBARU: u32  = 0x14;                     // rw, u32
const PCI_INT_LN: u32   = 0x3c;                     // rw, u8
const PCI_INT_PN: u32   = 0x3d;                     // ro, u8

const BIT0: u32 = 0b1;
const BIT1: u32 = 0b10;
const BIT2: u32 = 0b100;
const BIT3: u32 = 0b1000;
const BIT4: u32 = 0b10000;
const BIT5: u32 = 0b100000;
const BIT6: u32 = 0b1000000;
const BIT7: u32 = 0b10000000;
const BIT8: u32 = 0b100000000;
const BIT9: u32 = 0b1000000000;
const BIT10: u32 = 0b10000000000;
const BIT11: u32 = 0b100000000000;
const BIT12: u32 = 0b1000000000000;
const BIT13: u32 = 0b10000000000000;
const BIT14: u32 = 0b100000000000000;
const BIT15: u32 = 0b1000000000000000;
const BIT16: u32 = 0b10000000000000000;
const BIT17: u32 = 0b100000000000000000;
const BIT18: u32 = 0b1000000000000000000;
const BIT19: u32 = 0b10000000000000000000;
const BIT20: u32 = 0b100000000000000000000;
const BIT21: u32 = 0b1000000000000000000000;
const BIT22: u32 = 0b10000000000000000000000;
const BIT23: u32 = 0b100000000000000000000000;
const BIT24: u32 = 0b1000000000000000000000000;
const BIT25: u32 = 0b10000000000000000000000000;
const BIT26: u32 = 0b100000000000000000000000000;
const BIT27: u32 = 0b1000000000000000000000000000;
const BIT28: u32 = 0b10000000000000000000000000000;
const BIT29: u32 = 0b100000000000000000000000000000;
const BIT30: u32 = 0b1000000000000000000000000000000;
const BIT31: u32 = 0b10000000000000000000000000000000;

/// Memory Mapped configuration
/// Section 1.2, Intel I/O Controller Hub 7 Family External Design Specification, April 2005
const PCI_GCAP: u32         = 0x0; // ro, u16
const PCI_VMIN: u32         = 0x2; // ro, u8
const PCI_VMAJ: u32         = 0x3; // ro, u8
const PCI_OUTPAY: u32       = 0x4; // ro, u16
const PCI_INPAY: u32        = 0x6; // ro, u16
const PCI_GCTL: u32         = 0x8; // rw, u32
const PCI_WAKEEN: u32       = 0xb; // rw, u16
const PCI_STATESTS: u32     = 0xe; // rwc, u16
const PCI_GSTS: u32         = 0x10; // rwc, u16
const PCI_OUTSTRMPAY: u32   = 0x18; // ro, u16
const PCI_INSTRMPAY: u32    = 0x1a; // ro, u16
const PCI_INTCTL: u32       = 0x20; // rw, u32
const PCI_INTSTS: u32       = 0x24; // ro, u32
const PCI_WALCLK: u32       = 0x30; // ro, u32
const PCI_SSYNC_ICH6: u32   = 0x34; // rw, u32
const PCI_SSYNC_ICH10: u32  = 0x38; // rw, u32
// const PCI_SSYNC: u32        = PCI_SSYNC_ICH6;
const PCI_SSYNC: u32        = PCI_SSYNC_ICH10;
const PCI_CORBLBASE: u32    = 0x40; // rw, u32
const PCI_CORBUBASE: u32    = 0x44; // rw, u32
const PCI_CORBWP: u32       = 0x48; // rw, u16
const PCI_CORBRP: u32       = 0x4a; // rw, u16
const PCI_CORBCTL: u32      = 0x4c; // rw, u8
const PCI_CORBST: u32       = 0x4d; // rwc, u8
const PCI_CORBSIZE: u32     = 0x4e; // ro, u8
const PCI_RIRBLBASE: u32    = 0x50; // rw, u32
const PCI_RIRBUBASE: u32    = 0x54; // rw, u32
const PCI_RIRBWP: u32       = 0x58; // rw, u16
const PCI_RINTCNT: u32      = 0x5a; // rw, u16
const PCI_RIRBCTL: u32      = 0x5c; // rw, u8
const PCI_RIRBSTS: u32      = 0x5d; // rwc, u8
const PCI_RIRBSIZE: u32     = 0x5e; // ro, u8
const PCI_IC: u32           = 0x60; // rw, u32
const PCI_IR: u32           = 0x64; // ro, u32
const PCI_IRS: u32          = 0x68; // rwc, u16
const PCI_DPLBASE: u32      = 0x70; // rw, u32
const PCI_DPUBASE: u32      = 0x74; // rw, u32
const PCI_SDBASE: u32       = 0x80;
const PCI_SDSPAN: u32       = 0x20;

const PCI_SDCTL16: u32      = 0x0; // rw, u16
const PCI_SDCTL8: u32       = 0x2; // rw, u8
const PCI_SDSTS: u32        = 0x3; // rwc, u8
const PCI_SDLPIB: u32       = 0x4; // ro, u32
const PCI_SDCBL: u32        = 0x8; // rw, u32
const PCI_SDLVI: u32        = 0xc; // rw, u16
const PCI_SDFIFOW: u32      = 0xe; // rw, u16, ICH6 only
const PCI_SDFIFOS: u32      = 0x10; // rw, u16
const PCI_SDFMT: u32        = 0x12; // rw, u16
const PCI_SDBDPL: u32       = 0x18; // rw, u32
const PCI_SDBDPU: u32       = 0x1c; // rw, u32

const fn bitspan(h: usize, l: usize) -> u64 {
    (1 << h) | (((1 << h) - 1) & !((1 << l) - 1))
}

const PCI_CORBCTL_CMEI_BIT: u8 = BIT0 as u8;
const PCI_CORBCTL_DMA_BIT: u8 = BIT1 as u8;
const PCI_CORBRP_RST_BIT: u32 = BIT15;
const PCI_GCTL_RST_BIT: u32 = BIT0;
const PCI_GCTL_UNSOLICITED_BIT: u32 = BIT8;
const PCI_SDCTL8_STREAM_MASK: u8 = (BIT7 | BIT6 | BIT5 | BIT4) as u8;
const PCI_SDCTL8_STRIPE_MASK: u8 = (BIT1 | BIT0) as u8;
const PCI_SDCTL16_RSVDP_MASK: u16 = bitspan(15, 5) as u16;
const PCI_SDCTL16_DEI_BIT: u16 = BIT4 as u16;
const PCI_SDCTL16_FEI_BIT: u16 = BIT3 as u16;
const PCI_SDCTL16_INT_MASK: u16 = (BIT4 | BIT3 | BIT2) as u16;
const PCI_SDCTL16_IOC_BIT: u16 = BIT2 as u16;
const PCI_SDCTL16_RUN_BIT: u16 = BIT1 as u16;
const PCI_SDCTL16_SRST_BIT: u16 = BIT0 as u16;
const PCI_SDSTS_DEI_BIT: u8 = BIT4 as u8;
const PCI_SDSTS_FEI_BIT: u8 = BIT3 as u8;
const PCI_SDSTS_INT_MASK: u8 = (BIT4 | BIT3 | BIT2) as u8;
const PCI_SDSTS_IOC_BIT: u8 = BIT2 as u8;
const PCI_SDSTS_READY_BIT: u8 = BIT5 as u8;
const PCI_RIRBCTL_DMA_BIT: u8 = BIT1 as u8;
const PCI_RIRBCTL_INT_MASK: u8 = (BIT0 | BIT1 | BIT2) as u8;
const PCI_RIRBCTL_IRQ_BIT: u8 = BIT0 as u8;
const PCI_RIRBCTL_OVERRUN_BIT: u8 = BIT2 as u8;
const PCI_RIRBSTS_INT_MASK: u8 = (BIT0 | BIT2)as u8;
const PCI_RIRBSTS_OVERRUN_BIT: u8 = BIT2 as u8;
const PCI_RIRBSTS_RESPONSE_BIT: u8 = BIT0 as u8;
const PCI_RIRBWP_RST_BIT: u16 = BIT15 as u16;
const PCI_STATESTS_INT_MASK: u16 = (BIT0 | BIT1 | BIT2) as u16;
const PCI_STATESTS_SDI0_BIT: u16 = BIT0 as u16;
const PCI_STATESTS_SDI1_BIT: u16 = BIT1 as u16;
const PCI_STATESTS_SDI2_BIT: u16 = BIT2 as u16;
const PCI_SDBDPL_MASK: u32 = !(BIT0 | BIT1 | BIT2 | BIT3 | BIT4 | BIT5 | BIT6);
const PCI_SDLVI_RSVDP_MASK: u16 = bitspan(15, 8) as u16;
const PCI_SDFMT_RSVDP_MASK: u16 = BIT7 as u16;
const PCI_IRS_ICB_BIT: u16 = BIT0 as u16;
const PCI_IRS_IRV_BIT: u16 = BIT1 as u16;

const PCI_SDCTL8_STREAM_1_MASK: u8 = 1 << 4;
const PCI_SDCTL8_STREAM_2_MASK: u8 = 2 << 4;
const PCI_SDCTL8_STREAM_3_MASK: u8 = 3 << 4;
const PCI_SDCTL8_STREAM_4_MASK: u8 = 4 << 4;
const PCI_SDCTL8_STREAM_5_MASK: u8 = 5 << 4;
const PCI_SDCTL8_STREAM_6_MASK: u8 = 6 << 4;
const PCI_SDCTL8_STREAM_7_MASK: u8 = 7 << 4;
const PCI_SDCTL8_STREAM_8_MASK: u8 = 8 << 4;
const PCI_SDCTL8_STREAM_9_MASK: u8 = 9 << 4;
const PCI_SDCTL8_STREAM_10_MASK: u8 = 10 << 4;
const PCI_SDCTL8_STREAM_11_MASK: u8 = 11 << 4;
const PCI_SDCTL8_STREAM_12_MASK: u8 = 12 << 4;
const PCI_SDCTL8_STREAM_13_MASK: u8 = 13 << 4;
const PCI_SDCTL8_STREAM_14_MASK: u8 = 14 << 4;
const PCI_SDCTL8_STREAM_15_MASK: u8 = 15 << 4;

const PCI_SSYNC_RSVDP_MASK: u32 = BIT30 | BIT31;
const PCI_SSYNC_STREAM_MASK: u32 = !(PCI_SSYNC_RSVDP_MASK);

const PCI_INTSTS_SIS_MASK: u32 = !(PCI_INTSTS_CIS_BIT | PCI_INTSTS_GIS_BIT);
const PCI_INTSTS_CIS_BIT: u32 = BIT30;
const PCI_INTSTS_GIS_BIT: u32 = BIT31;

const PCI_INTCTL_SIE_MASK: u32 = !(PCI_INTCTL_CIE_BIT | PCI_INTCTL_GIE_BIT);
const PCI_INTCTL_CIE_BIT: u32 = BIT30;
const PCI_INTCTL_GIE_BIT: u32 = BIT31;

// 1.2.42 SDFMT
const PCM_FMT_44K_BIT: u16 = BIT14 as u16;

const PCM_FMT_DIV_1_MASK: u16 = 0 << 8;
const PCM_FMT_DIV_2_MASK: u16 = 1 << 8;
const PCM_FMT_DIV_3_MASK: u16 = 2 << 8;
const PCM_FMT_DIV_4_MASK: u16 = 3 << 8;
const PCM_FMT_DIV_5_MASK: u16 = 4 << 8;
const PCM_FMT_DIV_6_MASK: u16 = 5 << 8;
const PCM_FMT_DIV_7_MASK: u16 = 6 << 8;
const PCM_FMT_DIV_8_MASK: u16 = 7 << 8;

const PCM_FMT_MUL_1_MASK: u16 = 0 << 11;
const PCM_FMT_MUL_2_MASK: u16 = 1 << 11;
const PCM_FMT_MUL_3_MASK: u16 = 2 << 11;
const PCM_FMT_MUL_4_MASK: u16 = 3 << 11;

// Note that the alignment is 16 bits
const PCM_FMT_PACK_8_MASK: u16 = 0 << 4;
const PCM_FMT_PACK_16_MASK: u16 = 1 << 4;
// Note that the alignment is 32 bits
const PCM_FMT_PACK_20_MASK: u16 = 2 << 4;
const PCM_FMT_PACK_24_MASK: u16 = 3 << 4;
const PCM_FMT_PACK_32_MASK: u16 = 4 << 4;

const PCM_FMT_CHAN_MASK: u16 = 0xf;
const PCM_FMT_CHAN_1_MASK: u16 = 0;
const PCM_FMT_CHAN_2_MASK: u16 = 1;
const PCM_FMT_CHAN_3_MASK: u16 = 2;
const PCM_FMT_CHAN_4_MASK: u16 = 3;
const PCM_FMT_CHAN_5_MASK: u16 = 4;
const PCM_FMT_CHAN_6_MASK: u16 = 5;
const PCM_FMT_CHAN_7_MASK: u16 = 6;
const PCM_FMT_CHAN_8_MASK: u16 = 7;
const PCM_FMT_CHAN_9_MASK: u16 = 8;
const PCM_FMT_CHAN_10_MASK: u16 = 9;
const PCM_FMT_CHAN_11_MASK: u16 = 10;
const PCM_FMT_CHAN_12_MASK: u16 = 11;
const PCM_FMT_CHAN_13_MASK: u16 = 12;
const PCM_FMT_CHAN_14_MASK: u16 = 13;
const PCM_FMT_CHAN_15_MASK: u16 = 14;
const PCM_FMT_CHAN_16_MASK: u16 = 15;

const PCM_FMT_8000_MASK: u16 = PCM_FMT_MUL_1_MASK | PCM_FMT_DIV_6_MASK;
const PCM_FMT_11025_MASK: u16 = PCM_FMT_MUL_1_MASK | PCM_FMT_DIV_4_MASK | PCM_FMT_44K_BIT;
const PCM_FMT_16000_MASK: u16 = PCM_FMT_MUL_1_MASK | PCM_FMT_DIV_3_MASK;
const PCM_FMT_22050_MASK: u16 = PCM_FMT_MUL_1_MASK | PCM_FMT_DIV_2_MASK | PCM_FMT_44K_BIT;
const PCM_FMT_32000_MASK: u16 = PCM_FMT_MUL_2_MASK | PCM_FMT_DIV_3_MASK;
const PCM_FMT_44100_MASK: u16 = PCM_FMT_44K_BIT;
const PCM_FMT_48000_MASK: u16 = 0;
const PCM_FMT_88200_MASK: u16 = PCM_FMT_MUL_2_MASK | PCM_FMT_DIV_1_MASK | PCM_FMT_44K_BIT;
const PCM_FMT_96000_MASK: u16 = PCM_FMT_MUL_2_MASK | PCM_FMT_DIV_1_MASK;

static_assert!(PCM_FMT_22050_MASK | PCM_FMT_CHAN_2_MASK | PCM_FMT_PACK_16_MASK == 0b100000100010001);

const GCAP: IoBase<u16>         = IoBase::new(PCI_GCAP);
const VMIN: IoBase<u8>          = IoBase::new(PCI_VMIN);
const VMAJ: IoBase<u8>          = IoBase::new(PCI_VMAJ);
const OUTPAY: IoBase<u16>       = IoBase::new(PCI_OUTPAY);
const INPAY: IoBase<u16>        = IoBase::new(PCI_INPAY);
const GCTL: IoBase<u32>         = IoBase::new(PCI_GCTL);
const WAKEEN: IoBase<u16>       = IoBase::new(PCI_WAKEEN);
const STATESTS: IoBase<u16>     = IoBase::new(PCI_STATESTS);
const GSTS: IoBase<u16>         = IoBase::new(PCI_GSTS);
const OUTSTRMPAY: IoBase<u16>   = IoBase::new(PCI_OUTSTRMPAY);
const INSTRMPAY: IoBase<u16>    = IoBase::new(PCI_INSTRMPAY);
const INTCTL: IoBase<u32>       = IoBase::new(PCI_INTCTL);
const INTSTS: IoBase<u32>       = IoBase::new(PCI_INTSTS);
const WALCLK: IoBase<u32>       = IoBase::new(PCI_WALCLK);
const SSYNC: IoBase<u32>        = IoBase::new(PCI_SSYNC);
const CORBLBASE: IoBase<u32>    = IoBase::new(PCI_CORBLBASE);
const CORBUBASE: IoBase<u32>    = IoBase::new(PCI_CORBUBASE);
const CORBWP: IoBase<u16>       = IoBase::new(PCI_CORBWP);
const CORBRP: IoBase<u16>       = IoBase::new(PCI_CORBRP);
const CORBCTL: IoBase<u8>       = IoBase::new(PCI_CORBCTL);
const CORBST: IoBase<u8>        = IoBase::new(PCI_CORBST);
const CORBSIZE: IoBase<u8>      = IoBase::new(PCI_CORBSIZE);
const RIRBLBASE: IoBase<u32>    = IoBase::new(PCI_RIRBLBASE);
const RIRBUBASE: IoBase<u32>    = IoBase::new(PCI_RIRBUBASE);
const RIRBWP: IoBase<u16>       = IoBase::new(PCI_RIRBWP);
const RINTCNT: IoBase<u16>      = IoBase::new(PCI_RINTCNT);
const RIRBCTL: IoBase<u8>       = IoBase::new(PCI_RIRBCTL);
const RIRBSTS: IoBase<u8>       = IoBase::new(PCI_RIRBSTS);
const RIRBSIZE: IoBase<u8>      = IoBase::new(PCI_RIRBSIZE);
const IC: IoBase<u32>           = IoBase::new(PCI_IC);
const IR: IoBase<u32>           = IoBase::new(PCI_IR);
const IRS: IoBase<u16>          = IoBase::new(PCI_IRS);
const DPLBASE: IoBase<u32>      = IoBase::new(PCI_DPLBASE);
const DPUBASE: IoBase<u32>      = IoBase::new(PCI_DPUBASE);

const BDBAR_IOC_BIT: u32 = BIT0;

struct StreamRegisterSet {
    index: u32,
    base: u32,
}

impl StreamRegisterSet {
    fn new(index: u32) -> StreamRegisterSet {
        let base = PCI_SDBASE + PCI_SDSPAN * index as u32;
        StreamRegisterSet {
            index,
            base
        }
    }

    fn ctl16(&self) -> IoBase<u16> {
        IoBase::new(self.base + PCI_SDCTL16)
    }
    fn ctl8(&self) -> IoBase<u8> {
        IoBase::new(self.base + PCI_SDCTL8)
    }
    fn sts(&self) -> IoBase<u8> {
        IoBase::new(self.base + PCI_SDSTS)
    }
    fn lpib(&self) -> IoBase<u32> {
        IoBase::new(self.base + PCI_SDLPIB)
    }
    fn cbl(&self) -> IoBase<u32> {
        IoBase::new(self.base + PCI_SDCBL)
    }
    fn lvi(&self) -> IoBase<u16> {
        IoBase::new(self.base + PCI_SDLVI)
    }
    fn fifow(&self) -> IoBase<u16> {
        IoBase::new(self.base + PCI_SDFIFOW)
    }
    fn fifos(&self) -> IoBase<u16> {
        IoBase::new(self.base + PCI_SDFIFOS)
    }
    fn fmt(&self) -> IoBase<u16> {
        IoBase::new(self.base + PCI_SDFMT)
    }
    fn bdpl(&self) -> IoBase<u32> {
        IoBase::new(self.base + PCI_SDBDPL)
    }
    fn bdpu(&self) -> IoBase<u32> {
        IoBase::new(self.base + PCI_SDBDPU)
    }

    fn intctl_mask(&self) -> u32 {
        1 << self.index
    }

    fn ssync_mask(&self) -> u32 {
        1 << self.index
    }
}

fn out_stream(gcap: &GlobalCapabilities, index: usize) -> StreamRegisterSet {
    StreamRegisterSet::new(gcap.in_streams as u32 + index as u32)
}

fn in_stream(gcap: &GlobalCapabilities, index: usize) -> StreamRegisterSet {
    StreamRegisterSet::new(index as u32)
}

fn out_stream_1(device: &DeviceContext) -> StreamRegisterSet {
    StreamRegisterSet::new(device.in_streams as u32)
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct Descriptor {
    address: u64,             // physical address to sound data
    length: u32,              // The length of the buffer described in bytes (HDA Spec 1.0a)
    control: u32
}

// Minimum size is not specified by the minimum alignment is 128 bytes
const BUFFER_SIZE: usize = 4096;
// Maximum count is 256 according to the spec
const BUFFER_COUNT: usize = 32;

#[repr(C, align(128))]
#[derive(Copy, Clone)]
struct SampleBuffer {
    samples: [i16; BUFFER_SIZE]
}

// The alignment of 128 bytes is mandatory per the spec
#[repr(C, align(128))]
#[derive(Copy, Clone)]
struct BufferDescriptorListWithBuffers {
    descriptors: [Descriptor; BUFFER_COUNT],
    buffers: [SampleBuffer; BUFFER_COUNT]
}

struct DeviceContext {
    controller_handle: Handle,
    child_handle: Handle,
    driver_handle: Handle,
    audio_interface: SimpleAudioOut,
    in_streams: u32,
    out_streams: u32,
    codec: Codec,
}

struct EventGuard (uefi::Event);

impl EventGuard {
    fn wrap(event: uefi::Event) -> EventGuard {
        EventGuard (event)
    }

    fn unwrap(&self) -> uefi::Event {
        self.0
    }
}

impl Drop for EventGuard {
    fn drop(&mut self) {
        boot_services()
            .close_event(self.0)
            .expect_success("no good");
    }
}

struct PciMappingGuard<'a> {
    pci: &'a PciIO,
    mapping: Option<uefi::proto::pci::Mapping>
}

impl<'a> PciMappingGuard<'a> {
    fn wrap(pci: &'a PciIO, mapping: uefi::proto::pci::Mapping) -> PciMappingGuard<'a> {
        PciMappingGuard {
            pci,
            mapping: Some(mapping)
        }
    }

    fn unwrap(&self) -> &uefi::proto::pci::Mapping {
        self.mapping.as_ref().unwrap()
    }
}

impl<'a> Drop for PciMappingGuard<'a> {
    fn drop(&mut self) {
        let mapping = self.mapping.take().unwrap();
        info!("dropping PCI I/O mapping {:#x}", mapping.device_address());
        if let Err(error) = self.pci.unmap(mapping) {
            error!("unmap operation failed: {:?}", error.status());
        }
    }
}

// SAFETY: we only have a single thread so global mutable statics are safe
unsafe impl Sync for DeviceContext {}
unsafe impl Send for DeviceContext {}

static mut DEVICE_CONTEXTS: Option<alloc::vec::Vec<Box<DeviceContext>>> = None;

impl DeviceContext {
    // BootServices reference is only needed to inherit its lifetime
    fn from_protocol<'a>(_bs: &'a uefi::table::boot::BootServices, raw: *const SimpleAudioOut) -> Option<&'a DeviceContext> {
        unsafe {
            DEVICE_CONTEXTS
                .as_ref()
                .and_then(|contexts| {
                    contexts.iter().find(|&context| {
                        &context.audio_interface as *const SimpleAudioOut == raw
                    })
                })
                .map(|context| {
                    context.as_ref()
                })
        }
    }

    // BootServices reference is only needed to inhert its lifetime
    fn from_protocol_mut<'a>(_bs: &'a uefi::table::boot::BootServices, raw: *mut SimpleAudioOut) -> Option<&'a mut DeviceContext> {
        unsafe {
            DEVICE_CONTEXTS
                .as_deref_mut()
                .and_then(|contexts| {
                    contexts.iter_mut().find(|context| {
                        &context.audio_interface as *const SimpleAudioOut == raw as *const SimpleAudioOut
                    })
                })
                .map(|context| {
                    context.as_mut()
                })
        }
    }
}

fn register_device_context(device: Box<DeviceContext>) {
    unsafe {
        DEVICE_CONTEXTS
            .as_mut()
            .unwrap()
            .push(device)
            ;
    }
}

fn unregister_device_context(device: &DeviceContext) {
    unsafe {
        DEVICE_CONTEXTS
            .as_mut()
            .unwrap()
            .retain(|context| {
                &**context as *const DeviceContext != device as *const DeviceContext
            })
    }
}

fn bus_trace_registers(pci: &PciIO) -> uefi::Result {
    let rintcnt = RINTCNT.read(pci).ignore_warning()?;
    let gctl = GCTL.read(pci).ignore_warning()?;
    let statests = STATESTS.read(pci).ignore_warning()?;
    let intsts = INTSTS.read(pci).ignore_warning()?;
    let ssync = SSYNC.read(pci).ignore_warning()?;
    info!("GCTL:{:#010x} STATESTS:{:#010x} INTSTS:{:#010x} RINTCNT:0n{:08} SSYNC:{:#010x}",
          gctl, statests, intsts, rintcnt, ssync);
    uefi::Status::SUCCESS.into()
}

fn bus_clear_interrupt(pci: &PciIO) -> uefi::Result {
    let gcap = GCAP.read(pci)
        .ignore_warning()
        .map(GlobalCapabilities::from)?;
    for stream in 0..gcap.in_streams {
        in_stream(&gcap, stream as usize)
            .sts()
            .write(pci, PCI_SDSTS_INT_MASK)?;
    }
    for stream in 0..gcap.out_streams {
        out_stream(&gcap, stream as usize)
            .sts()
            .write(pci, PCI_SDSTS_INT_MASK)?;
    }
    STATESTS.write(pci, PCI_STATESTS_INT_MASK)?;
    RIRBSTS.write(pci, PCI_RIRBSTS_INT_MASK)?;
    INTSTS.write(pci, PCI_INTSTS_CIS_BIT | PCI_INTSTS_SIS_MASK)?;
    uefi::Status::SUCCESS.into()
}

struct GlobalCapabilities {
    out_streams: u16,
    in_streams: u16,
    bd_streams: u16,
    sdo_signals: u16,
    ok_64: bool,
}

impl GlobalCapabilities {
    fn from(gcap: u16) -> GlobalCapabilities {
        GlobalCapabilities {
            out_streams: (gcap >> 12) & 0b1111,
            in_streams: (gcap >> 8) & 0b1111,
            bd_streams: (gcap >> 3) & 0b11111,
            sdo_signals: (gcap >> 1) & 0b11,
            ok_64: (gcap as u32 & BIT0) == BIT0
        }
    }
}

fn bus_trace_config(pci: &PciIO) -> uefi::Result {
    let gcap = GCAP.read(pci)
        .ignore_warning()
        .map(GlobalCapabilities::from)?;
    info!("HDA controller Global Capabilities:");
    info!("  OSS:{}", gcap.out_streams);
    info!("  ISS:{}", gcap.in_streams);
    info!("  BID:{}", gcap.bd_streams);
    info!("  SDO:{}", gcap.sdo_signals);
    info!("  64k:{}", gcap.ok_64);
    uefi::Status::SUCCESS.into()
}

fn bus_reset(pci: &PciIO) -> uefi::Result<u16> {
    bus_stop(pci)?;
    let codec_mask = bus_start(pci).ignore_warning()?;
    bus_trace_config(pci)?;
    Ok(codec_mask.into())
}

fn bus_start(pci: &PciIO) -> uefi::Result<u16> {
    let codec_mask = bus_reset_link(pci).ignore_warning()?;
    bus_clear_interrupt(pci)?;
    bus_trace_registers(pci)?;
    INTCTL.or(pci, PCI_INTCTL_CIE_BIT | PCI_INTCTL_SIE_MASK)?;
    Ok(codec_mask.into())
}

fn bus_stop(pci: &PciIO) -> uefi::Result {
    let gcap = GCAP.read(pci)
        .ignore_warning()
        .map(GlobalCapabilities::from)?;
    // disabling interrupts for each stream descriptors is
    // only necessary when we taking ownership of the bus
    // link
    for stream in 0..gcap.in_streams {
        in_stream(&gcap, stream as usize)
            .ctl16()
            .and(pci, !PCI_SDCTL16_INT_MASK)?;
    }
    for stream in 0..gcap.out_streams {
        out_stream(&gcap, stream as usize)
            .ctl16()
            .and(pci, !PCI_SDCTL16_INT_MASK)?;
    }
    // disable SIE and GIE for all streams
    INTCTL.and(pci, !(PCI_INTCTL_SIE_MASK | PCI_INTCTL_CIE_BIT | PCI_INTCTL_GIE_BIT))?;
    bus_clear_interrupt(pci)?;
    uefi::Status::SUCCESS.into()
}

fn bus_reset_link(pci: &PciIO) -> uefi::Result<u16> {
    // disable interrupts is only necessary because someone might configured them (not me)
    STATESTS.write(pci, PCI_STATESTS_INT_MASK)?;
    // enter bus reset state
    GCTL.and(pci, !PCI_GCTL_RST_BIT)?;
    GCTL.wait(pci, INFINITY, PCI_GCTL_RST_BIT, 0)?;
    // leave bus reset state
    GCTL.or(pci, PCI_GCTL_RST_BIT)?;
    GCTL.wait(pci, INFINITY, PCI_GCTL_RST_BIT, PCI_GCTL_RST_BIT)?;
    // GCTL.RST is sticky so double check the bus state
    let gctl = GCTL.read(pci).ignore_warning()?;
    if gctl == 0 {
        info!("bus is not ready");
        return Err(uefi::Status::NOT_READY.into());
    }
    // TBD: detect codecs by timer or interrupt
    let codec_mask: u16 = STATESTS.read(pci).ignore_warning()?;
    info!("codec_mask:{:#b}", codec_mask);
    Ok(codec_mask.into())
}

fn bus_exec(pci: &PciIO, cmd: u32) -> uefi::Result<u32> {
    IRS.wait(pci, 10000, PCI_IRS_ICB_BIT, 0)?;
    IRS.or(pci, PCI_IRS_IRV_BIT)?;
    IC.write(pci, cmd)?;
    IRS.or(pci, PCI_IRS_ICB_BIT)?;
    IRS.wait(pci, 10000, (PCI_IRS_IRV_BIT | PCI_IRS_ICB_BIT), PCI_IRS_IRV_BIT)?;
    let result = IR.read(pci).ignore_warning()?;
    Ok(result.into())
}

#[derive(Clone, Copy, Debug)]
struct Param(u32);
#[derive(Clone, Copy, Debug)]
struct Codec(u32);
#[derive(Clone, Copy, Debug)]
struct Node(u32);
#[derive(Clone, Copy, Debug)]
struct Verb(u32);

const HDA_NODE_ROOT: Node = Node(0x0);

const HDA_VERB_PARAMS: Verb = Verb(0xf00);
const HDA_VERB_GET_CHANNEL_STREAM: Verb = Verb(0xf06);
const HDA_VERB_SET_CHANNEL_STREAM: Verb = Verb(0x706);
const HDA_VERB_SET_STREAM_FORMAT: Verb = Verb(0x200);
const HDA_VERB_GET_STREAM_FORMAT: Verb = Verb(0xa00);
const HDA_VERB_GET_CONFIG_DEFAULT: Verb = Verb(0xf1c);
const HDA_VERB_GET_PIN_WIDGET_CONTROL: Verb = Verb(0xf07);
const HDA_VERB_GET_CONNECTION_LIST: Verb = Verb(0xf02);
const HDA_VERB_GET_AMPLIFIER_GAIN_MUTE: Verb = Verb(0xb00);
const HDA_VERB_SET_AMPLIFIER_GAIN_MUTE: Verb = Verb(0x300);
const HDA_VERB_GET_EAPDBTL_ENABLE: Verb = Verb(0xf0c);
const HDA_VERB_SET_EAPDBTL_ENABLE: Verb = Verb(0x70c);
const HDA_VERB_SET_PIN_WIDGET_CONTROL: Verb = Verb(0x707);
const HDA_VERB_SET_POWER_STATE: Verb = Verb(0x705);
const HDA_VERB_GET_POWER_STATE: Verb = Verb(0xf05);
const HDA_VERB_GET_CONNECTION_SELECT: Verb = Verb(0xf01);
const HDA_VERB_SET_CONNECTION_SELECT: Verb = Verb(0x701);

const HDA_PARAM_VID: Param = Param(0x0);
const HDA_PARAM_NODE_COUNT: Param = Param(0x4);
const HDA_PARAM_AUDIO_WIDGET_CAPABILITIES: Param = Param(0x9);
const HDA_PARAM_FUNCTION_TYPE: Param = Param(0x5);
const HDA_PARAM_PIN_WIDGET_CAPABILITIES: Param = Param(0xc);
const HDA_PARAM_CONNECTION_LIST_LENGTH: Param = Param(0xe);
const HDA_PARAM_AMPLIFIER_OUTPUT_CAPABILITY: Param = Param(0x12);
const HDA_PARAM_SUPPORTED_POWER_STATES: Param = Param(0xf);
const HDA_PARAM_SUPPORTED_PCM: Param = Param(0xa);
const HDA_PARAM_VOLUME_KNOB_CAPABILITIES: Param = Param(0x13);

const HDA_FUNCTION_TYPE_MASK: u32 = BIT7-1;

const HDA_CONNECTION_LIST_LONG_BIT: u32 = BIT7;
const HDA_CONNECTION_LIST_LENGTH_MASK: u32 = BIT7-1;

const HDA_AUDIO_CAPABILITY_STEREO_BIT: u32 = BIT0;
const HDA_AUDIO_CAPABILITY_IN_AMP_PRESENT_BIT: u32 = BIT1;
const HDA_AUDIO_CAPABILITY_OUT_AMP_PRESENT_BIT: u32 = BIT2;
const HDA_AUDIO_CAPABILITY_AMP_PARAM_OVERRIDE_BIT: u32 = BIT3;
const HDA_AUDIO_CAPABILITY_FORMAT_OVERRIDE_BIT: u32 = BIT4;
const HDA_AUDIO_CAPABILITY_STRIPE_BIT: u32 = BIT5;
const HDA_AUDIO_CAPABILITY_PROC_WIDGET_BIT: u32 = BIT6;
const HDA_AUDIO_CAPABILITY_UNSOL_CAPABLE_BIT: u32 = BIT7;
const HDA_AUDIO_CAPABILITY_CONNECTION_LIST_BIT: u32 = BIT8;
const HDA_AUDIO_CAPABILITY_DIGITAL_BIT: u32 = BIT9;
const HDA_AUDIO_CAPABILITY_POWER_CTL_BIT: u32 = BIT10;
const HDA_AUDIO_CAPABILITY_LR_SWAP_BIT: u32 = BIT11;
const HDA_AUDIO_CAPABILITY_CP_CAPS_BIT: u32 = BIT12;
const HDA_AUDIO_CAPABILITY_CHAN_COUNT_EXT_MASK: u32 = BIT13 | BIT14 | BIT15;
const HDA_AUDIO_CAPABILITY_DELAY_MASK: u32 = BIT16 | BIT17 | BIT18 | BIT19;
const HDA_AUDIO_CAPABILITY_TYPE_MASK: u32 = BIT20 | BIT21 | BIT22 | BIT23;

const HDA_PIN_CAPABILITY_EAPDBTL_BIT: u32 = BIT16;
const HDA_PIN_EAPDBTL_EAPD_ENABLE_BIT: u32 = BIT1;
const HDA_PIN_EAPDBTL_BTL_ENABLE_BIT: u32 = BIT0;
const HDA_PIN_WIDGET_CONTROL_OUT_ENABLE_BIT: u32 = BIT6;
const HDA_PIN_WIDGET_CONTROL_IN_ENABLE_BIT: u32 = BIT5;

const HDA_AMPLIFIER_CAPABILITY_OFFSET_MASK: u32 = 0x7f;
const HDA_AMPLIFIER_CAPABILITY_NUMSTEPS_MASK: u32 = 0x7f00;
const HDA_AMPLIFIER_CAPABILITY_STEPSIZE_MASK: u32 = 0x7f0000;
const HDA_AMPLIFIER_CAPABILITY_MUTE_BIT: u32 = BIT31;

const HDA_AMPLIFIER_GAIN_MUTE_GAIN_MASK: u32 = 0x7f;
const HDA_AMPLIFIER_GAIN_MUTE_MUTE_BIT: u32 = BIT7;
const HDA_AMPLIFIER_GAIN_MUTE_INDEX_MASK: u32 = 0xf00;
const HDA_AMPLIFIER_GAIN_MUTE_SETR_BIT: u32 = BIT12;
const HDA_AMPLIFIER_GAIN_MUTE_SETL_BIT: u32 = BIT13;
const HDA_AMPLIFIER_GAIN_MUTE_SETI_BIT: u32 = BIT14;
const HDA_AMPLIFIER_GAIN_MUTE_SETO_BIT: u32 = BIT15;

const HDA_FUNCTION_AUDIO: u32 = 0x1;
const HDA_FUNCTION_MODEM: u32 = 0x2;

// Widget types
const HDA_WIDGET_AUDIO_OUT: u32 = 0;
const HDA_WIDGET_AUDIO_IN: u32 = 1;
const HDA_WIDGET_AUDIO_MIX: u32 = 2;
const HDA_WIDGET_AUDIO_SELECTOR: u32 = 3;
const HDA_WIDGET_PIN_COMPLEX: u32 = 4;
const HDA_WIDGET_POWER: u32 = 5;
const HDA_WIDGET_VOLUME_KNOB: u32 = 6;
const HDA_WIDGET_BEEP_GEN: u32 = 7;
const HDA_WIDGET_VENDOR: u32 = 0xf;

// Device types (0x0-0xf)
const HDA_JACK_LINE_OUT: u32 = 0x0;
const HDA_JACK_SPEAKER: u32 = 0x1;
const HDA_JACK_HP_OUT: u32 = 0x2;
const HDA_JACK_CD: u32 = 0x3;
const HDA_JACK_SPDIF_OUT: u32 = 0x4;
const HDA_JACK_DIG_OTHER_OUT: u32 = 0x5;
const HDA_JACK_MODEM_LINE_SIDE: u32 = 0x6;
const HDA_JACK_MODEM_HAND_SIDE: u32 = 0x7;
const HDA_JACK_LINE_IN: u32 = 0x8;
const HDA_JACK_AUX: u32 = 0x9;
const HDA_JACK_MIC_IN: u32 = 0xa;
const HDA_JACK_TELEPHONY: u32 = 0xb;
const HDA_JACK_SPDIF_IN: u32 = 0xc;
const HDA_JACK_DIG_OTHER_IN: u32 = 0xd;
const HDA_JACK_OTHER: u32 = 0xf;

// Port connectivity (0-3)
const HDA_JACK_PORT_COMPLEX: u32 = 0x0;
const HDA_JACK_PORT_NONE: u32 = 0x1;
const HDA_JACK_PORT_FIXED: u32 = 0x2;
const HDA_JACK_PORT_BOTH: u32 = 0x3;

// Supported power states
const HDA_POWER_STATE_D0: u32 = 0b000;
const HDA_POWER_STATE_D1: u32 = 0b001;
const HDA_POWER_STATE_D2: u32 = 0b010;
const HDA_POWER_STATE_D3HOT: u32 = 0b011;
const HDA_POWER_STATE_D3COLD: u32 = 0b100;

fn make_command(codec: Codec, node: Node, verb: Verb, param: Param) -> u32 {
    (codec.0 << 28) | (node.0 << 20) | (verb.0 << 8) | (param.0)
}

fn bus_probe_codecs(pci: &PciIO, codec_mask: u16) -> uefi::Result<alloc::vec::Vec<u32>> {
    let mut codecs = alloc::vec::Vec::new();
    // TBD: use STATESTS/codec_mask instead
    for codec in &[0, 1, 2, 3] {
        let cmd = make_command(Codec(*codec), HDA_NODE_ROOT, HDA_VERB_PARAMS, HDA_PARAM_VID);
        match bus_exec(pci, cmd).ignore_warning() {
            Ok(vid) => {
                info!("codec {:#x} is detected, response:{:#x}", codec, vid);
                codec_trace_config(pci, Codec(*codec))?;
                codecs.push(*codec);
            }
            Err(error) => {
                info!("codec {:#x} is not detected: {:?}", codec, error.status());
                // TBD: codec_mask might change there
                bus_reset(pci)?;
            }
        }
    }
    Ok(codecs.into())
}

fn stream_clear(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    // TBD: wait for PCI_SDCTL16_RUN_BIT to be gone
    out_stream_1(device)
        .ctl16()
        .and(pci, !(PCI_SDCTL16_RUN_BIT | PCI_SDCTL16_INT_MASK))?;
    out_stream_1(device)
        .sts()
        .write(pci, PCI_SDSTS_INT_MASK)?;
    out_stream_1(device)
        .ctl8()
        .and(pci, !PCI_SDCTL8_STRIPE_MASK)?;
    uefi::Status::SUCCESS.into()
}

fn stream_trace(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    let sd = out_stream_1(device);
    let ctl16 = sd.ctl16().read(pci).ignore_warning()?;
    let ctl8 = sd.ctl8().read(pci).ignore_warning()?;
    let fmt = sd.fmt().read(pci).ignore_warning()?;
    let cbl = sd.cbl().read(pci).ignore_warning()?;
    let lvi = sd.lvi().read(pci).ignore_warning()?;
    let bdpl = sd.bdpl().read(pci).ignore_warning()?;
    let bdpu = sd.bdpu().read(pci).ignore_warning()?;
    let sts = sd.sts().read(pci).ignore_warning()?;
    let lpib = sd.lpib().read(pci).ignore_warning()?;
    let fifow = sd.fifow().read(pci).ignore_warning()?;
    let fifos = sd.fifos().read(pci).ignore_warning()?;
    info!("stream_trace: ctl:{:#b}, fmt:{:#x}, cbl:{}, lvi:{}, bdp:{:#x}, sts:{:#b}, lpib:{}, fifow:{:#x}, fifos:{:#x}",
          ((ctl8 as u32) << 16) | ctl16 as u32,
          fmt, cbl, lvi,
          ((bdpu as u64) << 32) | bdpl as u64,
          sts, lpib, fifow, fifos);
    uefi::Status::SUCCESS.into()
}

fn codec_set_stream(device: &mut DeviceContext, pci: &PciIO, codec: Codec, node: Node) -> uefi::Result {
    let stream_id = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_CHANNEL_STREAM, Param(0x0)))
        .ignore_warning()?;
    info!("codec_set_stream: {:?} read: {:#x}", node, stream_id);
    bus_exec(pci, make_command(codec, node, HDA_VERB_SET_CHANNEL_STREAM, Param(PCI_SDCTL8_STREAM_1_MASK as u32)))?;
    let readback = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_CHANNEL_STREAM, Param(0x0)))
        .ignore_warning()?;
    info!("codec_set_stream: -- readback: {:#x}", readback);
    uefi::Status::SUCCESS.into()
}

fn pin_enable_eapd(device: &mut DeviceContext, pci: &PciIO, codec: Codec, node: Node, enable: bool) -> uefi::Result {
    let caps = bus_exec(pci, make_command(codec, node, HDA_VERB_PARAMS, HDA_PARAM_PIN_WIDGET_CAPABILITIES))
        .ignore_warning()?;
    if (caps & HDA_PIN_CAPABILITY_EAPDBTL_BIT) != 0 {
        let eapd = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_EAPDBTL_ENABLE, Param(0x0)))
            .ignore_warning()?;
        if enable {
            info!("pin_enable_eapd: {:?} enable EAPD/BTL (current {:#x})", node, eapd);
            bus_exec(pci, make_command(codec, node, HDA_VERB_SET_EAPDBTL_ENABLE, Param(eapd | (HDA_PIN_EAPDBTL_EAPD_ENABLE_BIT | HDA_PIN_EAPDBTL_BTL_ENABLE_BIT))))?;
        } else {
            info!("pin_enable_eapd: {:?} disable EAPD/BTL (current {:#x})", node, eapd);
            bus_exec(pci, make_command(codec, node, HDA_VERB_SET_EAPDBTL_ENABLE, Param(eapd & !(HDA_PIN_EAPDBTL_EAPD_ENABLE_BIT | HDA_PIN_EAPDBTL_BTL_ENABLE_BIT))))?;
        }
    }
    uefi::Status::SUCCESS.into()
}

fn pin_enable_output(device: &mut DeviceContext, pci: &PciIO, codec: Codec, node: Node, enable: bool) -> uefi::Result {
    let ctl = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_PIN_WIDGET_CONTROL, Param(0x0)))
        .ignore_warning()?;
    info!("pin_enable_output: {:?} enable: {}, ctl: {:#x}", node, enable, ctl);
    if enable {
        bus_exec(pci, make_command(codec, node, HDA_VERB_SET_PIN_WIDGET_CONTROL, Param(ctl | HDA_PIN_WIDGET_CONTROL_OUT_ENABLE_BIT)))?;
    } else {
        bus_exec(pci, make_command(codec, node, HDA_VERB_SET_PIN_WIDGET_CONTROL, Param(ctl & !HDA_PIN_WIDGET_CONTROL_OUT_ENABLE_BIT)))?;
    }
    // TBD: wait 100ms?
    boot_services().stall(milliseconds_to_stall(5));
    let readback = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_PIN_WIDGET_CONTROL, Param(0x0)))
        .ignore_warning()?;
    info!("pin_enable_output: -- readback: {:#x}", readback);
    uefi::Status::SUCCESS.into()
}

fn pin_power(device: &mut DeviceContext, pci: &PciIO, codec: Codec, node: Node, up: bool) -> uefi::Result {
    let power_state = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_POWER_STATE, Param(0x0)))
        .ignore_warning()?;
    info!("pin_power: {:?} up: {}, current: {:#x}", node, up, power_state);
    if up {
        bus_exec(pci, make_command(codec, node, HDA_VERB_SET_POWER_STATE, Param(HDA_POWER_STATE_D0)))?;
    } else {
        bus_exec(pci, make_command(codec, node, HDA_VERB_SET_POWER_STATE, Param(HDA_POWER_STATE_D3HOT)))?;
    }
    // TBD: do we need it at all?
    boot_services().stall(milliseconds_to_stall(5));
    let power_state = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_POWER_STATE, Param(0x0)))
        .ignore_warning()?;
    info!("pin_power: -- readback: {:#x}", power_state);
    if (power_state & BIT8) != 0 {
        return uefi::Status::DEVICE_ERROR.into();
    }
    uefi::Status::SUCCESS.into()
}

fn codec_set_format(device: &mut DeviceContext, pci: &PciIO, codec: Codec, node: Node, format: u16) -> uefi::Result {
    info!("codec_set_format: {:?} format: {:#x}", node, format);
    bus_exec(pci, make_command(codec, node, HDA_VERB_SET_STREAM_FORMAT, Param(format as u32)))?;
    let readback = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_STREAM_FORMAT, Param(0x0)))
        .ignore_warning()?;
    info!("codec_set_format -- readback: {:#x}", readback);
    uefi::Status::SUCCESS.into()
}

#[derive(Copy, Clone, Debug)]
struct NodeDescriptor {
    start_id: u32,
    count: u32
}

fn parse_node_count(response: u32) -> uefi::Result<NodeDescriptor> {
    if response & 0xffff_ffff == 0xffff_ffff {
        error!("error reading node count");
        return Err(uefi::Status::DEVICE_ERROR.into());
    }
    let start_id = (response >> 16) & 0x7fff;
    let count = response & 0x7fff;
    return Ok(NodeDescriptor { start_id, count }.into()).into();
}

fn find_audio_function_node(pci: &PciIO, codec: Codec) -> uefi::Result<Node> {
    let response = bus_exec(pci, make_command(codec, HDA_NODE_ROOT, HDA_VERB_PARAMS, HDA_PARAM_NODE_COUNT))
        .ignore_warning()?;
    let NodeDescriptor {start_id, count} = parse_node_count(response).ignore_warning()?;
    info!("sub nodes: {} nodes starting from {}", start_id, count);
    let mut afg = None;
    for n in start_id..(start_id + count) {
        let fun = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_FUNCTION_TYPE))
            .ignore_warning()?;
        if fun & HDA_FUNCTION_TYPE_MASK == HDA_FUNCTION_AUDIO {
            if afg.is_some() {
                warn!("Multiple AFG found! The other was {:?}", afg.unwrap());
            }
            afg = Some(Node(n));
        }
    }
    if afg.is_none() {
        error!("No AFG node found!");
        return Err(uefi::Status::NOT_FOUND.into());
    }
    Ok(afg.unwrap().into())
}

fn pin_mute_unmute(device: &mut DeviceContext, pci: &PciIO, codec: Codec, node: Node, mute: bool) -> uefi::Result {
    let amc = bus_exec(pci, make_command(codec, node, HDA_VERB_PARAMS, HDA_PARAM_AMPLIFIER_OUTPUT_CAPABILITY))
        .ignore_warning()?;
    let num_steps = (amc & HDA_AMPLIFIER_CAPABILITY_NUMSTEPS_MASK) >> 8;
    let offset = amc & HDA_AMPLIFIER_CAPABILITY_OFFSET_MASK;
    if amc & HDA_AMPLIFIER_CAPABILITY_MUTE_BIT != 0 {
        let mut flags = HDA_AMPLIFIER_GAIN_MUTE_SETO_BIT
            | HDA_AMPLIFIER_GAIN_MUTE_SETL_BIT
            | HDA_AMPLIFIER_GAIN_MUTE_SETR_BIT
            | offset;
        if mute {
            flags |= HDA_AMPLIFIER_GAIN_MUTE_MUTE_BIT;
            info!("mute {:?}", node);
        } else {
            info!("unmute {:?}", node);
        }
        bus_exec(pci, make_command(codec, node, HDA_VERB_SET_AMPLIFIER_GAIN_MUTE, Param(flags)))?;
    }
    Ok(().into())
}

fn pin_select(device: &mut DeviceContext, pci: &PciIO, codec: Codec, node: Node, index: usize) -> uefi::Result {
    let len = bus_exec(pci, make_command(codec, node, HDA_VERB_PARAMS, HDA_PARAM_CONNECTION_LIST_LENGTH))
        .ignore_warning()?;
    let select = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_CONNECTION_SELECT, Param(0x0)))
        .ignore_warning()?;
    info!("pin_select: {:?} is changing connection select from {} to {}", node, select, index);
    if index < len as usize {
        bus_exec(pci, make_command(codec, node, HDA_VERB_SET_CONNECTION_SELECT, Param(index as u32)))?;
        let select = bus_exec(pci, make_command(codec, node, HDA_VERB_GET_CONNECTION_SELECT, Param(0x0)))
            .ignore_warning()?;
        info!("pin_select: -- readback: {}", select);
    }
    Ok(().into())
}

fn codec_trace_config(pci: &PciIO, codec: Codec) -> uefi::Result {
    info!("codec_trace_config");
    let afg = find_audio_function_node(pci, codec).ignore_warning()?;
    info!("Audio FG is {:?}", afg);
    let ps_supported = bus_exec(pci, make_command(codec, afg, HDA_VERB_PARAMS, HDA_PARAM_SUPPORTED_POWER_STATES))
        .ignore_warning()?;
    let ps_current = bus_exec(pci, make_command(codec, afg, HDA_VERB_GET_POWER_STATE, Param(0x0)))
        .ignore_warning()?;
    let response = bus_exec(pci, make_command(codec, afg, HDA_VERB_PARAMS, HDA_PARAM_NODE_COUNT))
        .ignore_warning()?;
    let NodeDescriptor {start_id, count} = parse_node_count(response)
        .ignore_warning()?;
    info!("AFG {:?} has {} sub nodes starting with {}", afg, count, start_id);
    info!("AFG power state is {:#x} (supported {:#x})", ps_current, ps_supported);
    for n in start_id..(start_id + count) {
        let audio_caps = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_AUDIO_WIDGET_CAPABILITIES))
            .ignore_warning()?;
        let pin_caps = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_PIN_WIDGET_CAPABILITIES))
            .ignore_warning()?;
        let typ = widget_capabilities_type(audio_caps);
        let cfg = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_GET_CONFIG_DEFAULT, Param(0x0)))
            .ignore_warning()
            .map(PinConfig::from)?;
        let ctl = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_GET_PIN_WIDGET_CONTROL, Param(0x0)))
            .ignore_warning()?;
        let ps_current = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_GET_POWER_STATE, Param(0x0)))
            .ignore_warning()?;
        let ps_supported = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_SUPPORTED_POWER_STATES))
            .ignore_warning()?;
        let amc = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_AMPLIFIER_OUTPUT_CAPABILITY))
            .ignore_warning()?;
        let len = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_CONNECTION_LIST_LENGTH))
            .ignore_warning()?;
        let select = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_GET_CONNECTION_SELECT, Param(0x0)))
            .ignore_warning()?;
        let vol_knob = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_VOLUME_KNOB_CAPABILITIES))
            .ignore_warning()?;
        info!("{:?} has ctrl {:#x}", n, ctl);
        info!("{:?} has type {:#x}", n, typ);
        info!("{:?} has power state {:#x}", n, ps_current);
        info!("{:?} has supported power states {:#x}", n, ps_supported);
        info!("{:?} has volume knob capabilities {:#x}", n, vol_knob);
        info!("{:?} has connection list length {:#x}", n, len);
        info!("{:?} has connection select {}", n, select);
        if len & HDA_CONNECTION_LIST_LENGTH_MASK != 0 {
            info!("Connection List:");
            let mut step = 2;
            if len & HDA_CONNECTION_LIST_LONG_BIT == 0 {
                step += 2;
            }
            for i in (0..(len & HDA_CONNECTION_LIST_LENGTH_MASK)).step_by(step) {
                let mut cls = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_GET_CONNECTION_LIST, Param(i)))
                    .ignore_warning()?;
                for _ in 0..step {
                    info!("  {}: {}", i, cls & 0xff);
                    cls >>= 8;
                }
            }
        }
        info!("{:?} has amplifier capabilities {:#x}", n, amc);
        if amc != 0 {
            info!("Amplifier Capabilities:");
            if amc & BIT31 != 0 {
                info!("  mute");
            }
            let off = (amc >> 0) & 0x7f;
            info!("  off: {}", off);
            let num = (amc >> 8) & 0x7f;
            info!("  numsteps: {}", num);
            let size = (amc >> 16) & 0x7f;
            info!("  stepsize: {}", size);
        }
        if typ == HDA_WIDGET_PIN_COMPLEX {
            info!("Pin Complex:");
            info!("  pin_seq {:#x}", cfg.sequence);
            info!("  pin_ass {:#x}", cfg.association);
            info!("  pin_msc {:#x}", cfg.misc);
            info!("  pin_clr {:#x}", cfg.color);
            info!("  pin_typ {:#x}", cfg.typ);
            info!("  pin_dev {:#x}", cfg.device);
            info!("  pin_loc {:#x}", cfg.location);
            info!("  pin_prt {:#x}", cfg.port_connectivity);
        }
        if amc & HDA_AMPLIFIER_CAPABILITY_NUMSTEPS_MASK != 0 {
            // Get the output gain/mute params
            let amp = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_GET_AMPLIFIER_GAIN_MUTE, Param(BIT15)))
                .ignore_warning()?;
            info!("Amplifier Info");
            let mute = (amp >> 7) & 0x1;
            let gain = (amp >> 0) & 0x7f;
            info!("  amp_gain {:#x}", gain);
            info!("  amp_mute {:#x}", mute);
        }
        if pin_caps != 0 {
            info!("Pin Capabilities:");
            let mut cap = pin_caps;
            if cap & HDA_PIN_CAPABILITY_EAPDBTL_BIT != 0 {
                info!("  EAPD/BTL");
                cap &= !HDA_PIN_CAPABILITY_EAPDBTL_BIT;
            }
            if cap != 0 {
                info!("  Other bits: {:#x}", cap);
            }
        }
        if audio_caps != 0 {
            info!("Audio Widget Capabilities:");
            let mut caps = audio_caps;
            [(HDA_AUDIO_CAPABILITY_STEREO_BIT, "STEREO"),
             (HDA_AUDIO_CAPABILITY_IN_AMP_PRESENT_BIT, "IN_AMP_PRESENT"),
             (HDA_AUDIO_CAPABILITY_OUT_AMP_PRESENT_BIT, "OUT_AMP_PRESENT"),
             (HDA_AUDIO_CAPABILITY_AMP_PARAM_OVERRIDE_BIT, "AMP_PARAM_OVERRIDE"),
             (HDA_AUDIO_CAPABILITY_FORMAT_OVERRIDE_BIT, "FORMAT_OVERRIDE"),
             (HDA_AUDIO_CAPABILITY_STRIPE_BIT, "STRIPE"),
             (HDA_AUDIO_CAPABILITY_PROC_WIDGET_BIT, "PROC_WIDGET"),
             (HDA_AUDIO_CAPABILITY_UNSOL_CAPABLE_BIT, "UNSOL_CAPABLE"),
             (HDA_AUDIO_CAPABILITY_CONNECTION_LIST_BIT, "CONNECTION_LIST"),
             (HDA_AUDIO_CAPABILITY_DIGITAL_BIT, "DIGITAL"),
             (HDA_AUDIO_CAPABILITY_POWER_CTL_BIT, "POWER_CTL"),
             (HDA_AUDIO_CAPABILITY_LR_SWAP_BIT, "LR_SWAP"),
             (HDA_AUDIO_CAPABILITY_CP_CAPS_BIT, "CP_CAPS")
            ].iter().for_each(|(mask, name)| {
                if (caps & mask) != 0 {
                    caps &= !mask;
                    info!("  {}", name);
                }
            });
            info!("  Channel Count Ext: {}", (caps & HDA_AUDIO_CAPABILITY_CHAN_COUNT_EXT_MASK) >> 13);
            caps &= !HDA_AUDIO_CAPABILITY_CHAN_COUNT_EXT_MASK;
            info!("  Delay: {}", (caps & HDA_AUDIO_CAPABILITY_DELAY_MASK) >> 16);
            caps &= !HDA_AUDIO_CAPABILITY_DELAY_MASK;
            info!("  Type: {:#x}", (caps & HDA_AUDIO_CAPABILITY_TYPE_MASK) >> 20);
            caps &= !HDA_AUDIO_CAPABILITY_TYPE_MASK;
            if caps != 0 {
                info!("  Other bits: {:#x}", caps);
            }
        }
    }
    info!("codec_trace_config -- done");
    uefi::Status::SUCCESS.into()
}

fn codec_setup_stream(device: &mut DeviceContext, pci: &PciIO, codec: Codec, format: u16) -> uefi::Result {
    let afg = find_audio_function_node(pci, codec)
        .ignore_warning()?;
    let response = bus_exec(pci, make_command(codec, afg, HDA_VERB_PARAMS, HDA_PARAM_NODE_COUNT))
        .ignore_warning()?;
    let NodeDescriptor { start_id, count } = parse_node_count(response)
        .ignore_warning()?;
    info!("sub nodes: {} nodes starting from {}", start_id, count);
    pin_power(device, pci, codec, afg, true)?;
    for n in start_id..(start_id + count) {
        pin_power(device, pci, codec, Node(n), true)?;
        let cap = bus_exec(pci, make_command(codec, Node(n), HDA_VERB_PARAMS, HDA_PARAM_AUDIO_WIDGET_CAPABILITIES))
            .ignore_warning()?;
        let widget_type = widget_capabilities_type(cap);
        if widget_type == HDA_WIDGET_AUDIO_OUT {
            info!("Configure {:?} as Audio Out", Node(n));
            codec_set_stream(device, pci, codec, Node(n))?;
            codec_set_format(device, pci, codec, Node(n), format)?;
            pin_mute_unmute(device, pci, codec, Node(n), false)?;
        } else if widget_type == HDA_WIDGET_PIN_COMPLEX {
            info!("Configure {:?} as Pin Complex", Node(n));
            pin_enable_eapd(device, pci, codec, Node(n), true)?;
            pin_enable_output(device, pci, codec, Node(n), true)?;
            pin_mute_unmute(device, pci, codec, Node(n), false)?;
        } else if widget_type == HDA_WIDGET_AUDIO_MIX {
            info!("Configure {:?} as Audio Mix", Node(n));
            pin_enable_output(device, pci, codec, Node(n), true)?;
            pin_mute_unmute(device, pci, codec, Node(n), false)?;
        }
    }
    uefi::Status::SUCCESS.into()
}

struct PinConfig {
    sequence: u32,
    association: u32,
    misc: u32,
    color: u32,
    typ: u32,
    device: u32,
    location: u32,
    port_connectivity: u32,
}

impl PinConfig {
    fn from(cfg: u32) -> PinConfig {
        PinConfig {
            sequence: (cfg >> 0) & 0b1111,
            association: (cfg >> 4) & 0b1111,
            misc: (cfg >> 8) & 0xf,
            color: (cfg >> 12) & 0b1111,
            typ: (cfg >> 16) & 0b1111,
            device: (cfg >> 20) & 0b1111,
            location: (cfg >> 24) & 0b111111,
            port_connectivity: (cfg >> 30) & 0b11,
        }
    }
}

fn widget_capabilities_channels(caps: u32) -> u32 {
    2 * (((caps >> 13) & 0x7) + 1)
}

fn widget_capabilities_type(caps: u32) -> u32 {
    (caps >> 20) & 0xf
}

fn stream_cleanup(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    out_stream_1(device).bdpl().write(pci, 0)?;
    out_stream_1(device).bdpu().write(pci, 0)?;
    out_stream_1(device).ctl16().and(pci, !PCI_SDCTL16_RSVDP_MASK)?;
    out_stream_1(device).ctl8().write(pci, 0)?;
    uefi::Status::SUCCESS.into()
}

fn stream_reset(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    stream_clear(device, pci)?;
    // enter reset state
    out_stream_1(device).ctl16().or(pci, PCI_SDCTL16_SRST_BIT)?;
    out_stream_1(device).ctl16().wait(pci, 1000, PCI_SDCTL16_SRST_BIT, PCI_SDCTL16_SRST_BIT)?;
    // leave reset state
    out_stream_1(device).ctl16().and(pci, !PCI_SDCTL16_SRST_BIT)?;
    out_stream_1(device).ctl16().wait(pci, 1000, PCI_SDCTL16_SRST_BIT, 0)?;
    uefi::Status::SUCCESS.into()
}

fn stream_setup(device: &mut DeviceContext, pci: &PciIO, mapping: &uefi::proto::pci::Mapping, loop_buffers: u32, loop_samples: u32, format: u16) -> uefi::Result {
    info!("stream_setup, buffers: {}, samples: {}, format: {:#x}", loop_buffers, loop_samples, format);
    // TBD: make sure the run bit is zero for SD like so
    // stream_clear(device, pci)?;
    // set the stream tag
    out_stream_1(device)
        .ctl8()
        .write(pci, PCI_SDCTL8_STREAM_1_MASK)?;
    // the length of samples in cyclic buffer is in bytes
    out_stream_1(device)
        .cbl()
        .write(pci, loop_samples * mem::size_of::<i16>() as u32)?;
    // set the stream format
    let osd0fmt = out_stream_1(device)
        .fmt()
        .read(pci)
        .ignore_warning()?;
    out_stream_1(device)
        .fmt()
        .write(pci, (osd0fmt & PCI_SDFMT_RSVDP_MASK) | (format & !PCI_SDFMT_RSVDP_MASK))?;
    // set the stream LVI of the BDL
    let osd0lvi = out_stream_1(device)
        .lvi()
        .read(pci)
        .ignore_warning()?;
    out_stream_1(device)
        .lvi()
        .write(pci, (osd0lvi & PCI_SDLVI_RSVDP_MASK) | (loop_buffers as u16 - 1))?;
    // set the BDL address
    if ((mapping.device_address() & 0xffffffff) as u32 & !PCI_SDBDPL_MASK) != 0 {
        error!("mapping address is invalid {:#x}", mapping.device_address());
        return uefi::Status::INVALID_PARAMETER.into();
    }
    out_stream_1(device)
        .bdpl()
        .write(pci, (mapping.device_address() & 0xffffffff) as u32)?;
    out_stream_1(device)
        .bdpu()
        .write(pci, ((mapping.device_address() >> 32) & 0xffffffff) as u32)?;
    // enable all interrupts in SD though we dont use them at the moment
    out_stream_1(device)
        .ctl16()
        .or(pci, PCI_SDCTL16_INT_MASK)?;
    uefi::Status::SUCCESS.into()
}

fn stream_start(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    // enable SIE interrupt bit; we don't use interrupts atm
    INTCTL.or(pci, out_stream_1(device).intctl_mask())?;
    // set stripe to 0 even though it is meaningless for output streams
    out_stream_1(device)
        .ctl8()
        .and(pci, !PCI_SDCTL8_STRIPE_MASK)?;
    // start DMA; next step is to wait for SDSTS.FIFOREADY
    out_stream_1(device)
        .ctl16()
        .or(pci, PCI_SDCTL16_RUN_BIT | PCI_SDCTL16_INT_MASK)?;
    uefi::Status::SUCCESS.into()
}

fn stream_stop(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    stream_clear(device, pci)?;
    // disable SIE; we don't use interrupts atm
    INTCTL.and(pci, !out_stream_1(device).intctl_mask())?;
    uefi::Status::SUCCESS.into()
}

fn stream_sync_get(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    // TBD: SSYNC is ignored in qemu and meaningless for a
    // single stream
    SSYNC.or(pci, out_stream_1(device).ssync_mask())?;
    uefi::Status::SUCCESS.into()
}

fn stream_sync_put(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    // TBD: SSYNC is ignored in qemu and meaningless for a
    // single stream
    SSYNC.and(pci, !out_stream_1(device).ssync_mask())?;
    uefi::Status::SUCCESS.into()
}

fn stream_wait_sync_start(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    // TBD: a better way to check that FIFO is ready? prefill up to FIFOS bytes?
    // "For an Output stream, the controller hardware will
    // set this bit to a 1 while the output DMA FIFO
    // contains enough data to maintain the stream on the
    // link. This bit defaults to 0 on reset because the
    // FIFO is cleared on a reset. The amount of data
    // required to maintain the stream will depend on the
    // controller implementation but, in general, for an
    // output stream, it means that the FIFO is full."
    out_stream_1(device)
        .sts()
        .wait(pci, 1000, PCI_SDSTS_READY_BIT, PCI_SDSTS_READY_BIT)?;
    uefi::Status::SUCCESS.into()
}

fn stream_wait_sync_stop(device: &mut DeviceContext, pci: &PciIO) -> uefi::Result {
    out_stream_1(device)
        .ctl16()
        .wait(pci, 1000, PCI_SDCTL16_RUN_BIT, 0)?;
    uefi::Status::SUCCESS.into()
}

fn stream_loop<C>(device: &mut DeviceContext, pci: &PciIO, control: &mut C, sample_count: u64, channel_count: u64, sampling_rate: u64, duration: u64) -> uefi::Result
where C: DmaControl {
    let playback_event = boot_services()
        .create_timer_event()
        .ignore_warning()
        .map(EventGuard::wrap)?;
    let trace_event = boot_services()
        .create_timer_event()
        .ignore_warning()
        .map(EventGuard::wrap)?;
    // TBD: to prefill the buffers partially we must change
    //      LVI which is only possible if RUN bit is deasserted
    control.transfer(BUFFER_COUNT * BUFFER_SIZE);
    let playback_time = milliseconds_to_timer_period(duration);
    boot_services()
        .set_timer(
            playback_event.unwrap(),
            uefi::table::boot::TimerTrigger::Relative(playback_time))?;
    // TBD: this is basically called period length in alsa,
    //      maybe add as configuration parameter via
    //      DriverConfiguration?
    let delay = milliseconds_to_timer_period(sample_count / channel_count / sampling_rate);
    boot_services()
        .set_timer(
            trace_event.unwrap(),
            uefi::table::boot::TimerTrigger::Periodic(delay))?;
    stream_sync_get(device, pci);
    stream_start(device, pci);
    stream_wait_sync_start(device, pci);
    stream_sync_put(device, pci);
    let mut start_lpib = out_stream_1(device)
        .lpib()
        .read(pci)
        .ignore_warning()?;
    // number of slots in DMA cyclic buffer ready to be utilized
    let mut queue_room = 0;
    {
        loop {
            let actual_lpib = out_stream_1(device)
                .lpib()
                .read(pci)
                .ignore_warning()?;
            // info!("actual_lpib = {}, start_lpib = {}", actual_lpib, start_lpib);
            let room = if start_lpib <= actual_lpib {
                queue_room
                    + actual_lpib as usize / mem::size_of::<i16>()
                    - start_lpib as usize / mem::size_of::<i16>()
            } else {
                queue_room
                    + BUFFER_SIZE * BUFFER_COUNT
                    + actual_lpib as usize / mem::size_of::<i16>()
                    - start_lpib as usize / mem::size_of::<i16>()
            };
            if room as usize >= BUFFER_SIZE {
                let copied = control.transfer(room - room % BUFFER_SIZE);
                queue_room = room - copied;
                start_lpib = actual_lpib;
            }
            stream_trace(device, pci)?;
            // bus_trace_registers(pci)?;
            // Playback event must be placed first so that it
            // would be checked first
            let index = boot_services()
                .wait_for_event (&mut [playback_event.unwrap(), trace_event.unwrap()])
                .discard_errdata()?;
            if index.unwrap() == 0 {
                break;
            }
        }
        info!("stopping stream");
    }
    stream_sync_get(device, pci);
    stream_stop(device, pci);
    stream_wait_sync_stop(device, pci);
    stream_sync_put(device, pci);

    info!("clearning stream");
    stream_clear(device, pci);

    uefi::Status::SUCCESS.into()
}

fn stream_select_rate(device: &mut DeviceContext, pci: &PciIO, sampling_rate: u32, channel_count: u8) -> uefi::Result<(u16, u32)> {
    let abs_diff = |a, b| if a < b {b - a} else {a - b};
    // TBD: actually fill the list with only supported PCM formats
    let closest_rate =
        [
            AUDIO_RATE_8000,
            AUDIO_RATE_11025,
            AUDIO_RATE_16000,
            AUDIO_RATE_22050,
            AUDIO_RATE_32000,
            AUDIO_RATE_44100,
            AUDIO_RATE_48000,
        ]
        .iter()
        .min_by_key(|&&guess| abs_diff(guess, sampling_rate))
        .cloned()
        .ok_or_else(|| uefi::Status::UNSUPPORTED)?;
    if channel_count != 2 {
        return Err(uefi::Status::UNSUPPORTED.into());
    }
    let format = PCM_FMT_CHAN_2_MASK | PCM_FMT_PACK_16_MASK | match closest_rate {
        AUDIO_RATE_8000 => { PCM_FMT_8000_MASK }
        AUDIO_RATE_11025 => { PCM_FMT_11025_MASK },
        AUDIO_RATE_16000 => { PCM_FMT_16000_MASK },
        AUDIO_RATE_22050 => { PCM_FMT_22050_MASK },
        AUDIO_RATE_32000 => { PCM_FMT_32000_MASK },
        AUDIO_RATE_44100 => { PCM_FMT_44100_MASK },
        AUDIO_RATE_48000 => { PCM_FMT_48000_MASK },
        _ => unreachable!()
    };
    Ok((format, closest_rate).into())
}

// TBD: add format/channel converters
// TBD: add flow control/trottling/FIFOS handling
// TBD: use IOC bit to gracefully stop playback
trait DmaControl {
    fn transfer(&mut self, count: usize) -> usize;
}

struct Loop<'a> {
    samples: &'a [i16],
    bdl: &'a mut BufferDescriptorListWithBuffers,
    bdl_position: usize,
    samples_position: usize,
}

impl<'a> Loop<'a> {
    fn new(bdl: &'a mut BufferDescriptorListWithBuffers, samples: &'a [i16]) -> Loop<'a> {
        Loop {
            bdl,
            samples,
            bdl_position: 0,
            samples_position: 0
        }
    }
}

impl<'a> DmaControl for Loop<'a> {
    fn transfer(&mut self, count: usize) -> usize {
        info!("transfer: count = {}", count);
        let mut count = count;
        let mut total = 0;
        while count > 0 {
            info!(" transfer iteration: count = {}, bdl = {}, samples = {}", count, self.bdl_position, self.samples_position);
            let CopyResult {loop_buffers, loop_samples} =
                fill_bde(
                    &mut self.bdl.buffers[self.bdl_position],
                    &mut self.bdl.descriptors[self.bdl_position],
                    self.samples_position,
                    self.samples
                );
            info!("  transfer result: loop_samples = {}, loop_buffers = {}", loop_samples, loop_buffers);
            self.samples_position = (self.samples_position + loop_samples) % self.samples.len();
            self.bdl_position = (self.bdl_position + loop_buffers) % BUFFER_COUNT;
            count -= loop_samples;
            total += loop_samples;
        }
        total
    }
}

fn stream_play_loop(device: &mut DeviceContext, pci: &PciIO, duration: u64, samples: &[i16], sampling_rate: u32, channel_count: u8) -> uefi::Result {
    let mut bdl = Box::<BufferDescriptorListWithBuffers>::new_uninit();
    let mut bdl = unsafe { bdl.assume_init() };
    let mut bdl = &mut *bdl;

    let bdl_dma = pci
        .map(
            uefi::proto::pci::IoOperation::BusMasterWrite,
            &mut *bdl as *mut BufferDescriptorListWithBuffers as *mut _,
            mem::size_of::<BufferDescriptorListWithBuffers>())
        .map_err(|error| {error!("map operation failed: {:?}", error.status()); error})
        .ignore_warning()?;
    // Drop will unmap the memory buffer for us
    let bdl_dma = PciMappingGuard::wrap(pci, bdl_dma);

    let (format, closest_rate) = stream_select_rate(device, pci, sampling_rate, channel_count)
        .ignore_warning()?;

    info!("stream_play_loop: use {} sample rate", closest_rate);

    init_bdl(bdl_dma.unwrap(), bdl);

    let loop_buffers = BUFFER_COUNT;
    let loop_samples = BUFFER_COUNT * BUFFER_SIZE;

    let mut control = Loop::new(bdl, samples);

    // TBD: reset the stream? we could only modify CBL after _some_ reset
    codec_setup_stream(device, pci, device.codec, format)?;
    stream_setup(device, pci, bdl_dma.unwrap(), loop_buffers as u32, loop_samples as u32, format)?;

    stream_loop(device, pci, &mut control, samples.len() as u64, channel_count as u64, sampling_rate as u64, duration as u64)
        .map_err(|error| {
            stream_cleanup(device, pci).expect_success("double fail is unexpected");
            error
        })?;
    stream_cleanup(device, pci)?;
    uefi::Status::SUCCESS.into()
}

extern "efiapi" fn hda_tone(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> Status {
    info!("hda_tone");
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
    // Opening protocol with GET_PROTOCOL does not require
    // use to close protocol but if we do we will remove all
    // open protocol information from handle database (even
    // with different attributes, even with BY_DRIVER).
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            device.controller_handle,
            device.driver_handle,
            device.child_handle,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    pci.dont_close();
    let channel_count = 2;
    let sampling_rate = AUDIO_RATE_44100;
    let mut tone_samples = alloc::vec::Vec::new();
    tone_samples.resize(BUFFER_SIZE, 0);
    let sample_count = wave(tone_samples.as_mut_slice(), channel_count, sampling_rate, freq);
    tone_samples.truncate(sample_count);
    let samples = tone_samples.as_slice();
    // SAFETY: safe because no other references exist in our code
    let pci = unsafe { pci                           // OpenProtocol<'boot>
                       .as_proto()                   // &'boot UnsafeCell<PciIO>
                       .get()                        // *PciIO
                       .as_ref()                     // Option<&PciIO>
                       .unwrap() };
    stream_play_loop(device, pci, u64::from(duration), samples, sampling_rate, channel_count)?;
    info!("hda_tone -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_write(this: &mut SimpleAudioOut, sampling_rate: u32, channel_count: u8, format: u32, samples: *const i16, sample_count: usize) -> Status {
    info!("hda_write");
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
    // Opening protocol with GET_PROTOCOL does not require
    // use to close protocol but if we do we will remove all
    // open protocol information from handle database (even
    // with different attributes, even with BY_DRIVER).
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            device.controller_handle,
            device.driver_handle,
            device.child_handle,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    pci.dont_close();
    if channel_count != 2 {
        warn!("channel count {} is not supported!", channel_count);
        return uefi::Status::INVALID_PARAMETER;
    }
    if format != AUDIO_FORMAT_S16LE {
        warn!("format {:x} is not supported!", format);
        return uefi::Status::INVALID_PARAMETER;
    }
    if samples.is_null() || sample_count >= isize::MAX as usize {
        return uefi::Status::INVALID_PARAMETER;
    }
    // We check the alignment of the pointer as well because
    // this is generally enforced by EDK2
    if (samples as *mut u8 as usize) % mem::align_of::<i16>() != 0 {
        return uefi::Status::INVALID_PARAMETER;
    }
    // SAFETY: TBD
    let samples = unsafe { core::slice::from_raw_parts(samples, sample_count) };
    let duration_ms = 1000 * sample_count as u64 / channel_count as u64 / sampling_rate as u64;
    // SAFETY: safe because no other references exist in our code
    let pci = unsafe { pci                           // OpenProtocol<'boot>
                       .as_proto()                   // &'boot UnsafeCell<PciIO>
                       .get()                        // *PciIO
                       .as_ref()                     // Option<&PciIO>
                       .unwrap() };
    stream_play_loop(device, pci, duration_ms, samples, sampling_rate, channel_count)?;
    info!("hda_write -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_reset(this: &mut SimpleAudioOut) -> Status {
    info!("hda_reset");
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
    info!("hda_reset -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_query_mode(this: &mut SimpleAudioOut, index: usize, mode: &mut SimpleAudioMode) -> Status {
    info!("hda_query_mode");
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
    if index > 0 {
        // We only support a single mode -
        // Stereo|S16_LE|22050hz. Other modes can be
        // specified too in write() but they are not
        // guaranteed to work.
        warn!("Requested mode with index {} does not exist", index);
        return uefi::Status::INVALID_PARAMETER;
    }
    mode.sampling_rate = AUDIO_RATE_22050;
    mode.channel_count = 2;
    mode.sample_format = AUDIO_FORMAT_S16LE;
    info!("hda_query_mode -- ok");
    uefi::Status::SUCCESS
}

fn init_bdl(mapping: &uefi::proto::pci::Mapping, bdl: &mut BufferDescriptorListWithBuffers) {
    let bdl_base = bdl as *mut BufferDescriptorListWithBuffers as *mut u8;
    for (descriptor, buffer) in bdl.descriptors.iter_mut().zip(bdl.buffers.iter()) {
        // SAFETY: see dma-buffer miri test #1
        let buffer_offset = unsafe {
            (buffer.samples.as_ptr() as *const u8)
                .offset_from(bdl_base)
        };
        // TBD: UB if mapping address or bdl_base is not a valid pointer
        // SAFETY: TBD
        let descriptor_address = unsafe {
            (mapping.device_address() as *const u8)
                .offset(buffer_offset)
        };
        descriptor.address = descriptor_address as u64;
        descriptor.length = 0;
        descriptor.control = 0;
    }
}

fn init_context(driver_handle: Handle, controller_handle: Handle, pci: &PciIO, codec: Codec) -> uefi::Result<Box<DeviceContext>> {
    let gcap = GCAP.read(pci)
        .ignore_warning()
        .map(GlobalCapabilities::from)?;

    let device = Box::new(DeviceContext {
        controller_handle,
        child_handle: controller_handle,                 // TBD: no handle at the moment of context creation
        driver_handle,
        in_streams: gcap.in_streams as u32,
        out_streams: gcap.out_streams as u32,
        codec,
        audio_interface: SimpleAudioOut {
            reset: hda_reset,
            write: hda_write,
            tone: hda_tone,
            query_mode: hda_query_mode,
            max_mode: 1,
            capabilities: AUDIO_CAP_RESET | AUDIO_CAP_WRITE | AUDIO_CAP_TONE | AUDIO_CAP_MODE
        }
    });
    Ok (device.into())
}

fn deinit_context(pci: &PciIO) -> uefi::Result {
    Ok(().into())
}

#[derive(Copy, Clone)]
struct CopyResult {
    loop_buffers: usize,
    loop_samples: usize,
}

fn fill_bde(buffer: &mut SampleBuffer, descriptor: &mut Descriptor, samples_position: usize, samples: &[i16]) -> CopyResult {
    // Cycle through samples and fill entire buffer
    let mut samples_position = samples_position;
    let mut samples_to_copy = buffer.samples.len();
    let mut bdl_pos = 0;
    while samples_to_copy > 0 {
        let count = (samples.len() - samples_position).min(samples_to_copy);
        &mut buffer.samples[bdl_pos..bdl_pos+count]
            .copy_from_slice(&samples[samples_position..samples_position+count]);
        bdl_pos += count;
        samples_position = (samples_position + count) % samples.len();
        samples_to_copy -= count;
    }
    descriptor.length = buffer.samples.len() as u32 * mem::size_of::<i16>() as u32;
    descriptor.control = 0;
    CopyResult {
        loop_buffers: 1,
        loop_samples: buffer.samples.len()
    }
}

fn copy_samples_to_buffer(bdl: &mut BufferDescriptorListWithBuffers, index: usize, samples: &[i16]) -> CopyResult {
    let mut buffer_offset = 0;
    let mut buffer_count = 0;
    for (descriptor, buffer) in bdl.descriptors.iter_mut().zip(bdl.buffers.iter_mut()).skip(index) {
        let count = (samples.len() - buffer_offset).min(buffer.samples.len());
        &mut buffer.samples[0..count]
            .copy_from_slice(&samples[buffer_offset..buffer_offset+count]);
        // info!("copy_samples_to_buffer: schedule {} samples starting at {}", count, buffer_offset);
        if count > 0 {
            buffer_offset += count as usize;
            descriptor.length = count as u32 * mem::size_of::<i16>() as u32;
            descriptor.control = 0;
            buffer_count += 1;
        } else {
            descriptor.length = 0;
            descriptor.control = 0;
            // TBD: not ignored on qemu ich9-intel-hda
            // descriptor.control = BDBAR_IOC_BIT;
        }
    }
    CopyResult {
        loop_buffers: buffer_count,
        loop_samples: buffer_offset
    }
}

fn div_round_up(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

fn square(phase: usize, period: usize) -> i16 {
    if (phase % period) * 2 < period {
        i16::MAX
    } else {
        i16::MIN
    }
}

fn wave(buffer: &mut [i16], channels: u8, sampling_rate: u32, freq: u16) -> usize {
    if freq == 0 || channels == 0 || sampling_rate < freq as u32 {
        // TBD: other checks
        return 0;
    }
    // Get the number of full periods as a number of frames
    let samples_per_period      = channels as usize * sampling_rate as usize / freq as usize;
    let periods                 = buffer.len() / samples_per_period;
    let frames_per_period       = samples_per_period / channels as usize;
    let mut count               = 0;
    for period in buffer.chunks_mut(samples_per_period).take(periods) {
        for (index, v) in period.iter_mut().enumerate() {
            let frame_index = index / channels as usize;
            *v = square(frame_index, frames_per_period);
        }
        count += period.len();
    }
    count
}

fn milliseconds_to_timer_period(msec: u64) -> u64 {
    // Number of 100 ns units
    msec * 10000
}

fn milliseconds_to_stall(msec: usize) -> usize {
    // Number of microseconds
    msec * 1000
}

//
// DriverBinding routines
//

//
// PCI Vendor ID
//
const VID_INTEL: u16 = 0x8086;

const HDA_ICH9: u16 = 0x293e;
const HDA_ICH6: u16 = 0x2668;
const HDA_ICH7: u16 = 0x27d8;
const HDA_HM170: u16 = 0xa170;

extern "efiapi" fn hda_supported(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    // Opening the protocol BY_DRIVER results in
    // UNSUPPORTED, SUCCESS or ACCESS_DENIED. All must be
    // passed to boot manager.
    let pci = boot_services()
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .ignore_warning()?;
    info!("hda_supported -- got PCI");
    // SAFETY: safe because no other references exist in our code
    let pci = unsafe { pci                           // OpenProtocol<'boot>
                       .as_proto()                   // &'boot UnsafeCell<PciIO>
                       .get()                        // *PciIO
                       .as_ref()                     // Option<&PciIO>
                       .unwrap() };
    let vendor_id = pci.read_config_single::<u16>(PCI_VID)
        .ignore_warning()?;
    let device_id = pci.read_config_single::<u16>(PCI_DID)
        .ignore_warning()?;
    info!("vendor: {:#x}, device: {:#x}", vendor_id, device_id);
    // TBD: check class (4h) and subclass (3h)
    let supported = {
        [
            (VID_INTEL, HDA_ICH6), // ICH6
            (VID_INTEL, HDA_ICH7), // Intel ICH7 integrated HDA Controller
            (VID_INTEL, HDA_ICH9), // ICH9
            (VID_INTEL, HDA_HM170), // 100 Series/C230 Series Chipset Family HD Audio Controller
        ].iter().any(|&(vid, did)| {
            vendor_id == vid && device_id == did
        })
    };
    if !supported {
        return uefi::Status::UNSUPPORTED.into();
    }
    info!("hda_supported -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_start(this: &DriverBinding, controller_handle: Handle, remaining_path: *mut DevicePath) -> Status {
    info!("hda_start");
    // Sync with stop
    // SAFETY: when called by firmware we will be at notify or callback; for other cases we may
    //         as well check current TPL
    let _tpl = unsafe { boot_services().raise_tpl(uefi::table::boot::Tpl::NOTIFY) };
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            controller_handle,
            this.driver_handle(),
            controller_handle,
            OpenAttribute::BY_DRIVER)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    {
        // SAFETY: safe as long as no other references exist in our code
        let pci = unsafe { pci                           // OpenProtocol<'boot>
                           .as_proto()                   // &'boot UnsafeCell<PciIO>
                           .get()                        // *PciIO
                           .as_ref()                     // Option<&PciIO>
                           .unwrap() };

        let codec_mask = bus_reset(pci).ignore_warning()?;
        let gcap = GCAP.read(pci)
            .ignore_warning()
            .map(GlobalCapabilities::from)?;
        if gcap.out_streams == 0 {
            info!("No output streams supported!");
            return uefi::Status::UNSUPPORTED.into();
        }

        let detected_codecs = bus_probe_codecs(pci, codec_mask).ignore_warning()?;

        for codec in detected_codecs.into_iter() {
            bus_create_child(this.driver_handle(), controller_handle, pci, Codec(codec));
        }
    }

    // All children are created so now consume PCI I/O by this bus controller
    pci.dont_close();
    info!("hda_start -- ok");
    uefi::Status::SUCCESS
}

fn bus_create_child(driver_handle: Handle, controller_handle: Handle, pci: &PciIO, codec: Codec) -> uefi::Result {
    let mut device = init_context(driver_handle, controller_handle, pci, codec)
        .ignore_warning()?;
    let audio_out = &device.audio_interface;
    let child_handle = boot_services()
        .create_child::<SimpleAudioOut>(audio_out)
        .map_err(|error| {
            error!("failed to install audio protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    device.child_handle = child_handle;
    let result = boot_services()
        .open_protocol::<PciIO>(
            device.controller_handle,
            driver_handle,
            child_handle,
            OpenAttribute::BY_CHILD
        )
        .ignore_warning();
    match result {
        Err(error) => {
            error!("failed to open PCI I/O by child: {:?}", error.status());
            boot_services()
                .uninstall_interface::<SimpleAudioOut>(
                    child_handle,
                    audio_out);
            return error.status().into();
        }
        Ok(mut pci) => {
            pci.dont_close();
        }
    }
    // produce audio protocol and let it live in database as
    // long as the driver's image stay resident or until the
    // DisconnectController() will be invoked
    register_device_context(device);
    uefi::Status::SUCCESS.into()
}

fn hda_stop_bus(this: &DriverBinding, controller: Handle) -> uefi::Result {
    info!("hda_stop_bus");
    // SAFETY: its fine
    let _tpl = unsafe { boot_services().raise_tpl(uefi::table::boot::Tpl::NOTIFY) };
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            controller,
            this.driver_handle(),
            controller,
            OpenAttribute::BY_DRIVER)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    pci.dont_close();
    {
        // SAFETY: safe as long as no other references exist in our code
        let pci = unsafe { pci                           // OpenProtocol<'boot>
                           .as_proto()                   // &'boot UnsafeCell<PciIO>
                           .get()                        // *PciIO
                           .as_ref()                     // Option<&PciIO>
                           .unwrap() };
        bus_stop(pci)?;
    }
    pci.close()
        .map_err(|error| {
            error!("failed to close PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    info!("hda_stop_bus -- ok");
    uefi::Status::SUCCESS.into()
}

fn hda_stop_child(this: &DriverBinding, controller: Handle, child: Handle) -> uefi::Result {
    info!("hda_stop_child");
    // Sync with start
    // SAFETY: when called by firmware we will be at notify or callback; for other cases we may
    //         as well check current TPL
    let _tpl = unsafe { boot_services().raise_tpl(uefi::table::boot::Tpl::NOTIFY) };
    let audio_out = boot_services()
        .open_protocol::<SimpleAudioOut>(
            child,
            this.driver_handle(),
            controller,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open audio protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    // Opening protocol with GET_PROTOCOL does not require
    // use to close protocol but if we do we will remove all
    // open protocol information from handle database (even
    // with different attributes, even with BY_DRIVER).
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            controller,
            this.driver_handle(),
            controller,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    pci.dont_close();
    let audio_out = audio_out.as_proto().get();
    // SAFETY: safe as long as no other references exist in our code
    let audio_out_ref = unsafe { audio_out.as_ref().unwrap() };
    // Note that this operation does not consume anything
    let device = DeviceContext::from_protocol_mut(boot_services(), audio_out)
        .ok_or_else(|| uefi::Status::INVALID_PARAMETER)?;
    {
        // SAFETY: safe as long as no other references exist in our code
        let pci = unsafe { pci                           // OpenProtocol<'boot>
                           .as_proto()                   // &'boot UnsafeCell<PciIO>
                           .get()                        // *PciIO
                           .as_ref()                     // Option<&PciIO>
                           .unwrap() };
        deinit_context(pci)
            .map_err(|error| {
                warn!("Failed to deinitialize audio codec: {:?}", error.status());
                error
            })
            .or(uefi::Status::SUCCESS.into())?;                                       // ignore error
    }
    if let Err(status) = pci.close() {
        warn!("failed to close PCI I/O: {:?}", status);
    }
    boot_services()
        .uninstall_interface::<SimpleAudioOut>(child, audio_out_ref)
        .map_err(|error| {
            error!("failed uninstall audio protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    unregister_device_context(device);
    info!("hda_stop_child -- ok");
    Ok(().into())                                // drop audio
}

extern "efiapi" fn hda_stop_entry(this: &DriverBinding, controller: Handle, num_child_controller: usize, child_controller: *mut Handle) -> Status {
    if num_child_controller != 0 {
        if child_controller.is_null() {
            return uefi::Status::INVALID_PARAMETER;
        }
        let child_controllers = unsafe {
            core::slice::from_raw_parts(child_controller, num_child_controller)
        };
        for &child in child_controllers {
            hda_stop_child(this, controller, child)
                .ignore_warning()
                .map_err(|_| uefi::Status::DEVICE_ERROR.into())?;
        }
        uefi::Status::SUCCESS.into()
    } else {
        hda_stop_bus(this, controller)
            .status()
    }
}

//
// Image entry points
//

extern "efiapi" fn hda_unload(image_handle: Handle) -> Status {
    info!("hda_unload");
    let driver_binding = boot_services()
        .open_protocol::<DriverBinding>(
            image_handle,
            image_handle,
            image_handle,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open driver binding: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    // SAFETY: TBD
    let driver_binding_ref = unsafe { driver_binding.as_proto().get().as_ref().unwrap() };
    let handles = boot_services()
        .find_handles::<PciIO>()
        .log_warning()
        .map_err(|error| {
            warn!("failed to get PCI I/O handles: {:?}", error.status());
            error
        }).or_else(|error| {
            if error.status() == uefi::Status::NOT_FOUND {
                Ok(alloc::vec::Vec::new())
            } else {
                Err(error)
            }
        })?;
    for controller in handles {
        // If our drivers does not manage the specified
        // controller (i.e. does not hold it open BY_DRIVER)
        // then the disconnect will succeed
        let result = boot_services()
            .disconnect(
                controller,
                Some(driver_binding_ref.driver_handle()),
                None);
        if let Err(error) = result {
            warn!("failed to disconnect PCI I/O controller {:?}: {:?}", controller, error.status());
        }
    }
    if unsafe { !DEVICE_CONTEXTS.as_ref().unwrap().is_empty() } {
        error!("failed to disconnect some devices");
        return uefi::Status::DEVICE_ERROR.into();
    }
    boot_services()
        .uninstall_interface::<DriverBinding>(image_handle, driver_binding_ref)
        .map_err(|error| {
            error!("failed to uninstall driver binding: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    // Driver binding is already disconnected and it is
    // about to be destroyed so no need to close it.
    let driver_binding = uefi::table::boot::leak(driver_binding);
    // SAFETY: TBD
    unsafe { Box::from_raw(driver_binding.get()) };
    info!("hda_unload -- ok");
    // Cleanup allocator and logging facilities
    efi_dxe::unload(image_handle);
    uefi::Status::SUCCESS
}

#[entry]
fn efi_main(handle: uefi::Handle, system_table: SystemTable<Boot>) -> uefi::Status {
    efi_dxe::init(handle, &system_table)
        .ignore_warning()?;
    info!("hda_main");
    unsafe {
        DEVICE_CONTEXTS = Some(alloc::vec::Vec::new());
    }
    // TBD: allocate in runtime pool or .bss
    let driver_binding = Box::new(DriverBinding::new(
        hda_start,
        hda_supported,
        hda_stop_entry,
        0x0,
        handle,
        handle)
    );
    let loaded_image = boot_services()
        .handle_protocol::<LoadedImage>(handle)
        .ignore_warning()?;
    // SAFETY: TBD
    let loaded_image = unsafe { &mut *loaded_image.get() };
    loaded_image.set_unload_routine(Some(hda_unload));
    let driver_binding = Box::into_raw(driver_binding);
    // SAFETY: TBD
    let driver_binding_ref = unsafe { driver_binding.as_ref().unwrap() };
    boot_services()
        .install_interface::<DriverBinding>(handle, driver_binding_ref)
        .map_err(|error| {
            error!("failed to install driver binding: {:?}", error.status());
            // SAFETY: TBD
            unsafe { Box::from_raw(driver_binding) };
            error.status().into()
        })
        .ignore_warning()?;
    info!("initialization complete");
    boot_services()
        .handle_protocol::<DriverBinding>(handle)
        .ignore_warning()?;
    info!("hda_main -- ok");
    uefi::Status::SUCCESS
}