//! Named debug values (wasmcart's opt-in debug ABI, SPEC "Debug state"):
//! the table `wc_debug_state` returns, so a bot or test can read game state
//! by name. Values are copied in once a frame; host writes are ignored.

const U32: u8 = 4;
const F32: u8 = 6;
const MAX: usize = 40;
/// Words of value storage (arrays take several).
const WORDS: usize = 96;

#[repr(C)]
#[derive(Clone, Copy)]
struct Field {
    name_ptr: u32,
    value_ptr: u32,
    ty: u8,
    _pad: [u8; 3],
    len: u32,
}

const EMPTY: Field = Field { name_ptr: 0, value_ptr: 0, ty: 0, _pad: [0; 3], len: 0 };

struct Table {
    // One spare entry keeps the table NUL-terminated when full.
    fields: [Field; MAX + 1],
    values: [u32; WORDS],
    names: [&'static str; MAX],
    starts: [usize; MAX],
    count: usize,
    used: usize,
}

static mut TABLE: Table = Table {
    fields: [EMPTY; MAX + 1],
    values: [0; WORDS],
    names: [""; MAX],
    starts: [0; MAX],
    count: 0,
    used: 0,
};

fn table() -> &'static mut Table {
    // SAFETY: single threaded; the host reads the table between frames.
    unsafe { &mut *core::ptr::addr_of_mut!(TABLE) }
}

fn slot(name: &'static str, ty: u8, len: usize) -> Option<usize> {
    let t = table();
    if let Some(i) = t.names[..t.count].iter().position(|n| *n == name) {
        return Some(t.starts[i]);
    }
    if t.count == MAX || t.used + len > WORDS {
        return None;
    }
    let i = t.count;
    let mut z = String::with_capacity(name.len() + 1);
    z.push_str(name);
    z.push('\0');
    let z: &'static str = Box::leak(z.into_boxed_str());
    let start = t.used;
    t.names[i] = name;
    t.starts[i] = start;
    t.fields[i] = Field {
        name_ptr: z.as_ptr() as u32,
        value_ptr: core::ptr::addr_of!(t.values[start]) as u32,
        ty,
        _pad: [0; 3],
        len: len as u32,
    };
    t.count += 1;
    t.used += len;
    Some(start)
}

pub fn f32(name: &'static str, v: f32) {
    if let Some(s) = slot(name, F32, 1) {
        table().values[s] = v.to_bits();
    }
}
pub fn u32(name: &'static str, v: u32) {
    if let Some(s) = slot(name, U32, 1) {
        table().values[s] = v;
    }
}
pub fn f32s(name: &'static str, v: &[f32]) {
    if let Some(s) = slot(name, F32, v.len()) {
        for (k, x) in v.iter().enumerate() {
            table().values[s + k] = x.to_bits();
        }
    }
}

#[no_mangle]
pub extern "C" fn wc_debug_state() -> *const u8 {
    core::ptr::addr_of!(table().fields) as *const u8
}

/// Read back a u32 field (a host may have written it, e.g. "autoplay").
pub fn get_u32(name: &'static str) -> u32 {
    match slot(name, U32, 1) {
        Some(s) => table().values[s],
        None => 0,
    }
}
