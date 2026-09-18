use std::ffi::CString;
use std::path::Path;

const OK: i32 = 0;
const MAX_MIDI_BYTES: usize = 256 * 1024 * 1024;
const MAX_XMF_NODES: usize = 16_384;

unsafe extern "C" {
    fn kog_spessasynth_midi_convert(
        input: *const u8,
        input_size: usize,
        file_name: *const std::ffi::c_char,
        midi_data: *mut *mut u8,
        midi_size: *mut usize,
        title_data: *mut *mut u8,
        title_size: *mut usize,
    ) -> i32;
    fn kog_spessasynth_midi_free(data: *mut std::ffi::c_void);
}

#[derive(Debug)]
pub struct ConvertedMidi {
    pub bytes: Vec<u8>,
    pub title: Option<Vec<u8>>,
}

pub fn convert(bytes: &[u8], path: &Path) -> Result<ConvertedMidi, String> {
    if bytes.is_empty() {
        return Err("the MIDI container is empty".to_owned());
    }
    if bytes.len() > MAX_MIDI_BYTES {
        return Err("the MIDI container exceeds Kog's 256 MiB limit".to_owned());
    }
    if bytes.starts_with(b"XMF_") {
        validate_xmf_allocation_bounds(bytes)?;
    }
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let file_name = CString::new(file_name.as_bytes())
        .map_err(|_| "the MIDI container filename contains a NUL byte".to_owned())?;
    let mut midi_data = std::ptr::null_mut();
    let mut midi_size = 0_usize;
    let mut title_data = std::ptr::null_mut();
    let mut title_size = 0_usize;
    // SAFETY: the input slice and filename remain alive for the complete call.
    // The adapter initializes every output and returns independent malloc-backed
    // buffers, which are copied before being released with its matching free.
    let result = unsafe {
        kog_spessasynth_midi_convert(
            bytes.as_ptr(),
            bytes.len(),
            file_name.as_ptr(),
            &mut midi_data,
            &mut midi_size,
            &mut title_data,
            &mut title_size,
        )
    };
    if result != OK {
        return Err(result_message(result).to_owned());
    }

    let converted = copy_and_free(midi_data, midi_size);
    let title =
        (!title_data.is_null() && title_size > 0).then(|| copy_and_free(title_data, title_size));
    if title.is_none() && !title_data.is_null() {
        // The adapter currently never returns a non-null zero-length title,
        // but keep allocation ownership correct if that contract broadens.
        // SAFETY: this pointer came from the adapter and has not been freed.
        unsafe { kog_spessasynth_midi_free(title_data.cast()) };
    }
    if converted.is_empty() {
        return Err("SpessaSynth produced an empty Standard MIDI stream".to_owned());
    }
    Ok(ConvertedMidi {
        bytes: converted,
        title,
    })
}

fn validate_xmf_allocation_bounds(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 11 {
        return Err("the XMF header is truncated".to_owned());
    }
    let mut position = 8_usize;
    if bytes.get(4..8) == Some(b"2.00") {
        position = position
            .checked_add(8)
            .filter(|position| *position <= bytes.len())
            .ok_or_else(|| "the XMF 2.00 header is truncated".to_owned())?;
    }
    read_xmf_vlq(bytes, &mut position)?; // Informational file length.
    let metadata_size = read_xmf_vlq(bytes, &mut position)?;
    position = position
        .checked_add(metadata_size)
        .filter(|position| *position <= bytes.len())
        .ok_or_else(|| "the XMF file metadata exceeds the input".to_owned())?;
    let tree_start = read_xmf_vlq(bytes, &mut position)?;
    let root = bytes
        .get(tree_start..)
        .ok_or_else(|| "the XMF root node is outside the input".to_owned())?;
    let mut node_count = 0_usize;
    let mut decoded_total = 0_usize;
    validate_xmf_node(root, true, &mut node_count, &mut decoded_total)
}

fn validate_xmf_node(
    node: &[u8],
    is_root: bool,
    node_count: &mut usize,
    decoded_total: &mut usize,
) -> Result<(), String> {
    *node_count = node_count
        .checked_add(1)
        .filter(|count| *count <= MAX_XMF_NODES)
        .ok_or_else(|| "the XMF tree exceeds Kog's 16,384-node limit".to_owned())?;
    let mut position = 0_usize;
    let declared_length = read_xmf_vlq(node, &mut position)?;
    let node_length = if is_root && (declared_length == 0 || declared_length > node.len()) {
        node.len()
    } else {
        declared_length
    };
    if node_length == 0 || node_length > node.len() {
        return Err("an XMF node length exceeds the input".to_owned());
    }
    let item_count = read_xmf_vlq(node, &mut position)?;
    let header_size = read_xmf_vlq(node, &mut position)?;
    if header_size > node_length {
        return Err("an XMF node header exceeds its node".to_owned());
    }
    let metadata_size = read_xmf_vlq(node, &mut position)?;
    position = position
        .checked_add(metadata_size)
        .filter(|position| *position <= header_size)
        .ok_or_else(|| "an XMF node metadata table exceeds its header".to_owned())?;

    let unpackers_start = position;
    let unpackers_size = read_xmf_vlq(node, &mut position)?;
    let unpackers_end = unpackers_start
        .checked_add(unpackers_size)
        .filter(|end| *end <= header_size)
        .ok_or_else(|| "an XMF unpacker block exceeds its header".to_owned())?;
    if unpackers_size > 0 {
        while position < unpackers_end {
            match read_xmf_vlq(node, &mut position)? {
                0 => {
                    read_xmf_vlq(node, &mut position)?;
                }
                1 => {
                    let manufacturer = *node
                        .get(position)
                        .ok_or_else(|| "an XMF MMA unpacker is truncated".to_owned())?;
                    position += 1;
                    if manufacturer == 0 {
                        position = position
                            .checked_add(2)
                            .filter(|position| *position <= unpackers_end)
                            .ok_or_else(|| "an XMF MMA manufacturer ID is truncated".to_owned())?;
                    }
                    read_xmf_vlq(node, &mut position)?;
                }
                _ => return Err("the XMF file uses an unsupported unpacker".to_owned()),
            }
            let decoded_size = read_xmf_vlq(node, &mut position)?;
            *decoded_total = decoded_total
                .checked_add(decoded_size)
                .filter(|total| *total <= MAX_MIDI_BYTES)
                .ok_or_else(|| "the XMF decoded payloads exceed Kog's 256 MiB limit".to_owned())?;
        }
        if position != unpackers_end {
            return Err("an XMF unpacker record exceeds its block".to_owned());
        }
    }

    position = header_size;
    let reference_type = read_xmf_vlq(node, &mut position)?;
    if reference_type != 1 {
        return Err("the XMF file uses an unsupported external resource reference".to_owned());
    }
    if position > node_length {
        return Err("an XMF resource reference exceeds its node".to_owned());
    }
    if item_count == 0 {
        return Ok(());
    }

    let payload = &node[position..node_length];
    let mut child_position = 0_usize;
    let mut children_seen = 0_usize;
    while child_position < payload.len() && children_seen < item_count {
        let child_start = child_position;
        let child_length = read_xmf_vlq(payload, &mut child_position)?;
        let child_end = child_start
            .checked_add(child_length)
            .filter(|end| child_length > 0 && *end <= payload.len())
            .ok_or_else(|| "an XMF child node exceeds its folder".to_owned())?;
        validate_xmf_node(
            &payload[child_start..child_end],
            false,
            node_count,
            decoded_total,
        )?;
        child_position = child_end;
        children_seen += 1;
    }
    Ok(())
}

fn read_xmf_vlq(bytes: &[u8], position: &mut usize) -> Result<usize, String> {
    let mut value = 0_usize;
    for _ in 0..5 {
        let byte = *bytes
            .get(*position)
            .ok_or_else(|| "an XMF variable-length value is truncated".to_owned())?;
        *position += 1;
        value = value
            .checked_mul(128)
            .and_then(|value| value.checked_add(usize::from(byte & 0x7f)))
            .ok_or_else(|| "an XMF variable-length value overflows this platform".to_owned())?;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("an XMF variable-length value exceeds 32 bits".to_owned())
}

fn copy_and_free(data: *mut u8, size: usize) -> Vec<u8> {
    if data.is_null() || size == 0 {
        return Vec::new();
    }
    // SAFETY: successful conversion returns `size` initialized bytes. The
    // allocation stays live until the matching adapter free below.
    let bytes = unsafe { std::slice::from_raw_parts(data, size) }.to_vec();
    // SAFETY: the pointer was allocated by the adapter and is released once.
    unsafe { kog_spessasynth_midi_free(data.cast()) };
    bytes
}

fn result_message(result: i32) -> &'static str {
    match result {
        1 => "invalid SpessaSynth MIDI conversion arguments",
        2 => "SpessaSynth could not allocate a MIDI stream",
        3 => "SpessaSynth rejected the MIDI container",
        4 => "SpessaSynth could not serialize the converted MIDI stream",
        5 => "the converted MIDI stream exceeds Kog's 256 MiB limit",
        6 => "SpessaSynth ran out of memory while converting MIDI",
        _ => "SpessaSynth returned an unknown MIDI conversion error",
    }
}
