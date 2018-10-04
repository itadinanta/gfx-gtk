use shared_library::dynamic_library::DynamicLibrary;
use std::path::Path;
use std::ptr;

type LibPtr = *const std::ffi::c_void;

pub trait ProcLoader {
	fn get_proc_addr(&self, s: &str) -> Option<LibPtr>;
}

pub fn fn_from<P>(loader: P) -> impl Fn(&str) -> LibPtr
where
	P: ProcLoader + Sized,
{
	move |s| loader.get_proc_addr(s).unwrap_or_else(|| ptr::null())
}

pub struct DlProcLoader {
	lib: Option<shared_library::dynamic_library::DynamicLibrary>,
}

impl DlProcLoader {
	pub fn open(lib_path: &Path) -> Self {
		DlProcLoader {
			lib: DynamicLibrary::open(Some(lib_path)).ok(),
		}
	}
	pub fn current_module() -> Self {
		DlProcLoader {
			lib: DynamicLibrary::open(None).ok(),
		}
	}
}

impl ProcLoader for DlProcLoader {
	fn get_proc_addr(&self, s: &str) -> Option<LibPtr> {
		self.lib
			.as_ref()
			.and_then(|l| match unsafe { l.symbol(s) } {
				Ok(v) => Some(v as LibPtr),
				Err(_) => None,
			})
	}
}

pub struct Failover<A, B>(pub A, pub B)
where
	A: ProcLoader,
	B: ProcLoader;

impl<A, B> ProcLoader for Failover<A, B>
where
	A: ProcLoader,
	B: ProcLoader,
{
	fn get_proc_addr(&self, s: &str) -> Option<LibPtr> {
		self.0.get_proc_addr(s).or_else(|| self.1.get_proc_addr(s))
	}
}

pub fn debug_get_proc_addr(s: &str) -> LibPtr {
	let v = epoxy::get_proc_addr(s);
	if v.is_null() {
		println!("Symbol not found: {}", s);
	} else {
		println!("Loaded symbol: {} @ {:?}", s, v);
	}
	v
}
