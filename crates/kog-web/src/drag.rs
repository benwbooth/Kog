use wasm_bindgen::JsValue;

thread_local! {
    static TRACK_DROP: js_sys::Function = js_sys::Function::new_with_args(
        "x,y,queueLength", include_str!("track-drop.js"));
}

pub fn track_drop_target(x: i32, y: i32, queue_length: usize) -> Option<usize> {
    TRACK_DROP.with(|function| {
        function
            .call3(
                &JsValue::NULL,
                &x.into(),
                &y.into(),
                &JsValue::from_f64(queue_length as f64),
            )
            .ok()
            .and_then(|value| value.as_f64())
            .map(|index| index as usize)
    })
}

pub fn clear_track_drop_marker() {
    let _ = js_sys::eval(
        "document.querySelectorAll('.track.reorder-above, .track.reorder-below').forEach(row => row.classList.remove('reorder-above', 'reorder-below'))",
    );
}
