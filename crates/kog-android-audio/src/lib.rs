use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use jni::JNIEnv;
use jni::objects::{JByteArray, JObject, JString};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jint, jlong};
use kog_audio::decoder::{DecoderSettings, PlaybackSource};
use kog_audio::streaming::PcmReader;

struct Handle {
    reader: PcmReader,
    scratch: Vec<u8>,
}

static HANDLES: OnceLock<Mutex<HashMap<jlong, Arc<Mutex<Handle>>>>> = OnceLock::new();
static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);

fn handles() -> &'static Mutex<HashMap<jlong, Arc<Mutex<Handle>>>> {
    HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_handle(id: jlong) -> Option<Arc<Mutex<Handle>>> {
    handles().lock().ok()?.get(&id).cloned()
}

fn fail(env: &mut JNIEnv, message: impl AsRef<str>) {
    let _ = env.throw_new("java/io/IOException", message.as_ref());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_kog_player_NativeAudio_nativeSetHelperDirectory(
    mut env: JNIEnv,
    _receiver: JObject,
    path: JString,
) -> jboolean {
    let path: String = match env.get_string(&path) {
        Ok(path) => path.into(),
        Err(error) => {
            fail(&mut env, error.to_string());
            return JNI_FALSE;
        }
    };
    #[cfg(target_os = "android")]
    if let Err(error) = kog_audio::android_helpers::set_directory(PathBuf::from(path)) {
        fail(&mut env, error);
        return JNI_FALSE;
    }
    #[cfg(not(target_os = "android"))]
    let _ = path;
    JNI_TRUE
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_kog_player_NativeAudio_nativeOpen(
    mut env: JNIEnv,
    _receiver: JObject,
    path: JString,
    subsong: jint,
) -> jlong {
    let path: String = match env.get_string(&path) {
        Ok(path) => path.into(),
        Err(error) => {
            fail(&mut env, error.to_string());
            return 0;
        }
    };
    let path = PathBuf::from(path);
    let reader = if subsong >= 0 {
        let mut source = PlaybackSource::from_path(path);
        source.subsong = Some(subsong as u32);
        PcmReader::open(source, DecoderSettings::default())
    } else {
        PcmReader::open_path(path, DecoderSettings::default())
    };
    match reader {
        Ok(reader) => {
            let id = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            match handles().lock() {
                Ok(mut all) => {
                    all.insert(
                        id,
                        Arc::new(Mutex::new(Handle {
                            reader,
                            scratch: Vec::new(),
                        })),
                    );
                    id
                }
                Err(_) => {
                    fail(&mut env, "Decoder registry is unavailable");
                    0
                }
            }
        }
        Err(error) => {
            fail(&mut env, error);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_kog_player_NativeAudio_nativeDurationMs(
    _env: JNIEnv,
    _receiver: JObject,
    handle: jlong,
) -> jlong {
    get_handle(handle)
        .and_then(|handle| handle.lock().ok().and_then(|guard| guard.reader.duration()))
        .map_or(-1, |duration| duration.as_millis() as jlong)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_kog_player_NativeAudio_nativeRead(
    mut env: JNIEnv,
    _receiver: JObject,
    handle: jlong,
    output: JByteArray,
    offset: jint,
    length: jint,
) -> jint {
    if offset < 0 || length < 0 {
        fail(&mut env, "Invalid decoder read");
        return -1;
    }
    let Ok(capacity) = env.get_array_length(&output) else {
        fail(&mut env, "Invalid output buffer");
        return -1;
    };
    if offset > capacity || length > capacity - offset {
        fail(&mut env, "Output buffer is too small");
        return -1;
    }
    let Some(handle) = get_handle(handle) else {
        fail(&mut env, "Decoder is closed");
        return -1;
    };
    let Ok(mut handle) = handle.lock() else {
        fail(&mut env, "Decoder is unavailable");
        return -1;
    };
    let samples = (length as usize) / 2;
    if samples == 0 {
        return 0;
    }
    handle.scratch.resize(samples * 4, 0);
    let read = {
        let Handle { reader, scratch } = &mut *handle;
        reader.read(scratch)
    };
    let read = match read {
        Ok(read) => read,
        Err(error) => {
            fail(&mut env, error.to_string());
            return -1;
        }
    };
    if read == 0 {
        return -1;
    }
    let mut pcm = Vec::with_capacity((read / 4) * 2);
    for chunk in handle.scratch[..read].chunks_exact(4) {
        let value = f32::from_le_bytes(chunk.try_into().expect("four-byte chunk"));
        let sample = (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        pcm.extend(sample.to_le_bytes().map(|byte| byte as i8));
    }
    if let Err(error) = env.set_byte_array_region(&output, offset, &pcm) {
        fail(&mut env, error.to_string());
        return -1;
    }
    pcm.len() as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_kog_player_NativeAudio_nativeSeek(
    mut env: JNIEnv,
    _receiver: JObject,
    handle: jlong,
    position_ms: jlong,
) -> jboolean {
    if position_ms < 0 {
        fail(&mut env, "Invalid decoder seek");
        return JNI_FALSE;
    }
    let Some(handle) = get_handle(handle) else {
        fail(&mut env, "Decoder is closed");
        return JNI_FALSE;
    };
    let Ok(mut handle) = handle.lock() else {
        fail(&mut env, "Decoder is unavailable");
        return JNI_FALSE;
    };
    match handle
        .reader
        .seek(Duration::from_millis(position_ms as u64))
    {
        Ok(()) => JNI_TRUE,
        Err(error) => {
            fail(&mut env, error);
            JNI_FALSE
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_kog_player_NativeAudio_nativeClose(
    _env: JNIEnv,
    _receiver: JObject,
    handle: jlong,
) {
    if let Ok(mut all) = handles().lock() {
        all.remove(&handle);
    }
}
