extern crate shared_library;

use std::ptr;

fn dynamic_library_get_proc_addr(s: &str) -> *const std::ffi::c_void {
	unsafe {
		match shared_library::dynamic_library::DynamicLibrary::open(None)
			.unwrap()
			.symbol(s)
		{
			Ok(v) => v,
			Err(e) => {
				println!("`{}` {:?}", s, e);
				ptr::null()
			}
		}
	}
}

#[no_mangle]
pub fn notfound() -> &'static str {
	"do not optimize"
}

fn main() {
	dynamic_library_get_proc_addr("notfound");
	print!("{}", notfound());
}
