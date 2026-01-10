// FICHIER : src-wasm/blocks/governance/src/lib.rs

use serde_json::{json, Value};
use std::mem;
use std::slice;
use std::str;

// --- LOGIQUE MÉTIER ---

fn logic(input: Value) -> Value {
    // On récupère la valeur du capteur (simulée ou réelle)
    let vibration = input
        .get("sensor_vibration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    // SEUIL DYNAMIQUE (C'est ici qu'on peut changer la règle sans recompiler le moteur)
    // Seuil fixé à 9.5
    if vibration > 9.5 {
        json!({
            "approved": false,
            "reason": format!("GOUVERNANCE WASM: Vibration excessive ({:.2} > 9.5)", vibration)
        })
    } else {
        json!({
            "approved": true,
            "reason": "GOUVERNANCE WASM: Paramètres nominaux"
        })
    }
}

// --- PLOMBERIE WASM (ABI) ---
// Ne pas modifier cette partie, elle sert au moteur Rust pour parler au WASM

#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn process(ptr: *mut u8, len: usize) -> u64 {
    let data = slice::from_raw_parts(ptr, len);
    let input_str = str::from_utf8(data).unwrap();
    let input: Value = serde_json::from_str(input_str).unwrap();

    let output = logic(input);

    let output_str = output.to_string();
    let output_bytes = output_str.as_bytes();
    let out_len = output_bytes.len();
    let out_ptr = alloc(out_len);

    std::ptr::copy_nonoverlapping(output_bytes.as_ptr(), out_ptr, out_len);
    ((out_ptr as u64) << 32) | (out_len as u64)
}

// --- TESTS UNITAIRES (Ceux qui manquaient !) ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vibration_ok() {
        let input = json!({ "sensor_vibration": 5.0 });
        let result = logic(input);
        assert_eq!(result["approved"], true);
    }

    #[test]
    fn test_vibration_trop_haute() {
        let input = json!({ "sensor_vibration": 10.0 });
        let result = logic(input);
        assert_eq!(result["approved"], false);
        assert!(result["reason"].as_str().unwrap().contains("9.5"));
    }
}
