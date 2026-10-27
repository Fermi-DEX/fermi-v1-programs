use std::cell::{Ref, RefMut};

use anchor_lang::prelude::*;

                                                                                 
                                                                                    
pub trait DynamicHeader: Sized {
                                                                                 
    fn from_bytes(dynamic_data: &[u8]) -> Result<Self>;

                                                                    
    fn initialize(dynamic_data: &mut [u8]) -> Result<()>;
}

#[derive(Clone)]
pub struct DynamicAccount<Header, Fixed, Dynamic> {
    pub header: Header,
    pub fixed: Fixed,
    pub dynamic: Dynamic,
}

                           
                           
                                                
                               
pub trait DerefOrBorrow<T: ?Sized> {
    fn deref_or_borrow(&self) -> &T;
}

impl<T: ?Sized> DerefOrBorrow<T> for T {
    fn deref_or_borrow(&self) -> &T {
        self
    }
}

impl<T: ?Sized> DerefOrBorrow<T> for &T {
    fn deref_or_borrow(&self) -> &T {
        self
    }
}

impl<T: Sized> DerefOrBorrow<[T]> for Vec<T> {
    fn deref_or_borrow(&self) -> &[T] {
        self
    }
}

impl<T: ?Sized> DerefOrBorrow<T> for &mut T {
    fn deref_or_borrow(&self) -> &T {
        self
    }
}

impl<'a, T: ?Sized> DerefOrBorrow<T> for Ref<'a, T> {
    fn deref_or_borrow(&self) -> &T {
        self
    }
}

impl<'a, T: ?Sized> DerefOrBorrow<T> for RefMut<'a, T> {
    fn deref_or_borrow(&self) -> &T {
        self
    }
}

pub trait DerefOrBorrowMut<T: ?Sized> {
    fn deref_or_borrow_mut(&mut self) -> &mut T;
}

impl<T: ?Sized> DerefOrBorrowMut<T> for T {
    fn deref_or_borrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<T: ?Sized> DerefOrBorrowMut<T> for &mut T {
    fn deref_or_borrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<'a, T: ?Sized> DerefOrBorrowMut<T> for RefMut<'a, T> {
    fn deref_or_borrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<T: Sized> DerefOrBorrowMut<[T]> for Vec<T> {
    fn deref_or_borrow_mut(&mut self) -> &mut [T] {
        self
    }
}
