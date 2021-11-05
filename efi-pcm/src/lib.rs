#![no_std]
#![feature(abi_efiapi)]

// necessary for derive(Protocol) in our crate
#![feature(negative_impls)]

extern crate log;
extern crate uefi;

// TBD: these must be located on the root crate so that unsafe_guid macro will work
// for our proto module. Would be better to modify unsafe_guid macro to
// use module local definitions for Guid and Identify
use uefi::Guid;
use uefi::Identify;

// TBD: additional module is necessary due to uefi-rs inconsistencies
mod proto;
pub use proto::*;
