#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(improper_ctypes)] // silencing warning about 128 bit types
#![allow(unused_variables)]

#[allow(unsafe_op_in_unsafe_fn)]
pub mod bindings {
        include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
