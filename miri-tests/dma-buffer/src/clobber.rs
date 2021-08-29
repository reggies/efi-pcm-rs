#![feature(asm)]


extern "C" {
    fn clobber(ptr: &mut *const ());
}


// #[inline(never)]
// extern "C" fn clobber(ptr: &mut *const ()) {
//     let mut tmp = null();
//     unsafe {
//         asm!("/* {0} */", out(reg) tmp);
//     }
//     *ptr = tmp;
// }
