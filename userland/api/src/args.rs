/// Reads one NUL-terminated argument from the process startup vector.
///
/// # Safety
/// `argv` must point to an array of at least `count` readable pointers, and
/// every non-null entry must point to readable memory containing a NUL byte
/// within 256 bytes.
pub unsafe fn argument<'a>(argv: *const *const u8, count: usize, index: usize) -> Option<&'a [u8]> {
    if index >= count || argv.is_null() {
        return None;
    }
    // SAFETY: contratado pelo chamador (`argv` válido com `count` entradas).
    let pointer = unsafe { *argv.add(index) };
    unsafe { c_string(pointer) }
}

/// # Safety
/// `envp` must point to an array of at least `count` readable pointers.
pub unsafe fn environment_value<'a>(
    envp: *const *const u8,
    count: usize,
    name: &[u8],
) -> Option<&'a [u8]> {
    for index in 0..count {
        // SAFETY: contratado pelo chamador (ver doc da função).
        let entry = unsafe { argument(envp, count, index) };
        let Some(entry) = entry else {
            continue;
        };
        if entry.starts_with(name) && entry.get(name.len()) == Some(&b'=') {
            return Some(&entry[name.len() + 1..]);
        }
    }
    None
}

/// Resolve `path` contra o diretório corrente (`PWD`), devolvendo o caminho
/// absoluto em `output`.
pub fn resolve_path(cwd: Option<&[u8]>, path: &[u8], output: &mut [u8]) -> Option<usize> {
    if path.starts_with(b"/") {
        if path.len() > output.len() {
            return None;
        }
        output[..path.len()].copy_from_slice(path);
        return Some(path.len());
    }

    let cwd = cwd.filter(|value| !value.is_empty()).unwrap_or(b"/");
    let separator = usize::from(!cwd.is_empty() && cwd.last() != Some(&b'/'));
    let length = cwd.len().checked_add(separator)?.checked_add(path.len())?;
    if length > output.len() {
        return None;
    }
    output[..cwd.len()].copy_from_slice(cwd);
    let mut cursor = cwd.len();
    if separator != 0 {
        output[cursor] = b'/';
        cursor += 1;
    }
    output[cursor..length].copy_from_slice(path);
    Some(length)
}

pub fn append_path_component(parent: &[u8], component: &[u8], output: &mut [u8]) -> Option<usize> {
    let separator = usize::from(!parent.is_empty() && parent.last() != Some(&b'/'));
    let component_start = parent.len().checked_add(separator)?;
    let length = component_start.checked_add(component.len())?;
    if length > output.len() {
        return None;
    }
    output[..parent.len()].copy_from_slice(parent);
    if separator != 0 {
        output[parent.len()] = b'/';
    }
    output[component_start..length].copy_from_slice(component);
    Some(length)
}

unsafe fn c_string<'a>(pointer: *const u8) -> Option<&'a [u8]> {
    if pointer.is_null() {
        return None;
    }
    let mut length = 0usize;
    // SAFETY: contratado pelo chamador: o ponteiro aponta para memória
    // legível com um NUL nos primeiros 256 bytes.
    while length < 256 && unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    Some(unsafe { core::slice::from_raw_parts(pointer, length) })
}
