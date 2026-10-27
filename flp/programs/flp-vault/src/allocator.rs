                                                               
   
                                                                               
                                                                              
                                                                               
                                                                          
                                                                   
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout};

                                                                              
               
pub const HEAP_END_ADDRESS: usize = 0x400000000;

#[cfg(all(target_os = "solana", feature = "custom-heap", not(feature = "no-entrypoint")))]
#[global_allocator]
pub static ALLOCATOR: BumpAllocator = BumpAllocator {};

pub struct BumpAllocator {}

unsafe impl GlobalAlloc for BumpAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let heap_start = anchor_lang::solana_program::entrypoint::HEAP_START_ADDRESS as usize;
        let pos_ptr = heap_start as *mut usize;
        let mut pos = *pos_ptr;
        if pos == 0 {
                                                                            
            pos = heap_start + 8;
        }
        let mask = layout.align().wrapping_sub(1);
        let begin = pos.wrapping_add(mask) & !mask;
        let end = begin.checked_add(layout.size()).unwrap();
        *pos_ptr = end;
        assert!(end < HEAP_END_ADDRESS);
                                                                    
        *(end as *mut u8) = 0;
        begin as *mut u8
    }
    #[inline]
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
