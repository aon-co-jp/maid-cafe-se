//! maid-cafe-se Android版のRustコア。Kotlinの`tokyo.runo.maidcafese.core.Native`から呼ばれるJNI関数。
//!
//! 計算は[`bridge`](純Rust、JVMなしでテストできる)にあり、ここは型の出し入れだけ。
//! Rust側でpanicしても、JVMを巻き込んで落とさない(`null`/空を返す)。

pub mod bridge;

use jni::objects::{JByteArray, JClass, JFloatArray, JIntArray, JObject, JObjectArray, JString};
use jni::sys::{jboolean, jdouble, jfloatArray, jint, jlong, jshortArray, jstring};
use jni::JNIEnv;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::null_mut;

fn read_string(env: &mut JNIEnv, s: &JString) -> Option<String> {
    env.get_string(s).ok().map(|j| j.into())
}

fn to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s).map(|j| j.into_raw()).unwrap_or(null_mut())
}

/// panicを止めて`None`にする。
fn guarded<T>(f: impl FnOnce() -> Option<T>) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f)).ok().flatten()
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_planNext<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entries: JString<'l>,
    events: JString<'l>,
    settings: JString<'l>,
    after: jlong,
) -> jstring {
    let out = guarded(|| {
        let (e, v, s) = (read_string(&mut env, &entries)?, read_string(&mut env, &events)?, read_string(&mut env, &settings)?);
        Some(bridge::plan_next(&e, &v, &s, after))
    });
    to_jstring(&mut env, &out.unwrap_or_default())
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_alarmSpeech<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    kind: JString<'l>,
    text: JString<'l>,
    label: JString<'l>,
    voice: JString<'l>,
    ids: JString<'l>,
) -> jstring {
    let out = guarded(|| {
        let args = [&kind, &text, &label, &voice, &ids].map(|s| read_string(&mut env, s));
        let [Some(kind), Some(text), Some(label), Some(voice), Some(ids)] = args else { return None };
        bridge::alarm_speech(&kind, &text, &label, &voice, &ids)
    });
    match out {
        Some(s) => to_jstring(&mut env, &s),
        None => null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_phraseEdit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    numbers: JString<'l>,
    op: JString<'l>,
    id: JString<'l>,
    n: jint,
) -> jstring {
    let out = guarded(|| {
        let (nums, op, id) = (read_string(&mut env, &numbers)?, read_string(&mut env, &op)?, read_string(&mut env, &id)?);
        bridge::phrase_edit(&nums, &op, &id, n).ok()
    });
    match out {
        Some(s) => to_jstring(&mut env, &s),
        None => null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_phraseOrder<'l>(mut env: JNIEnv<'l>, _class: JClass<'l>, numbers: JString<'l>) -> jstring {
    let out = guarded(|| Some(bridge::phrase_order(&read_string(&mut env, &numbers)?)));
    to_jstring(&mut env, &out.unwrap_or_default())
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_phraseRanks<'l>(mut env: JNIEnv<'l>, _class: JClass<'l>, ids: JString<'l>) -> jstring {
    let out = guarded(|| Some(bridge::phrase_ranks(&read_string(&mut env, &ids)?)));
    to_jstring(&mut env, &out.unwrap_or_default())
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_wavSampleRate<'l>(env: JNIEnv<'l>, _class: JClass<'l>, wav: JByteArray<'l>) -> jint {
    guarded(|| env.convert_byte_array(&wav).ok().map(|b| bridge::wav_sample_rate(&b))).unwrap_or(0)
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_renderSegment<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    wav: JByteArray<'l>,
    voice: JString<'l>,
    gender: JString<'l>,
    harmony: jboolean,
    pitch: jdouble,
) -> jfloatArray {
    let samples = guarded(|| {
        let bytes = env.convert_byte_array(&wav).ok()?;
        let (voice, gender) = (read_string(&mut env, &voice)?, read_string(&mut env, &gender)?);
        bridge::render_segment(&bytes, &voice, &gender, harmony != 0, pitch)
    });
    let Some(samples) = samples else { return null_mut() };
    let Ok(arr) = env.new_float_array(samples.len() as jint) else { return null_mut() };
    if env.set_float_array_region(&arr, 0, &samples).is_err() {
        return null_mut();
    }
    arr.into_raw()
}

#[no_mangle]
pub extern "system" fn Java_tokyo_runo_maidcafese_core_Native_joinPcm16<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    parts: JObjectArray<'l>,
    gaps: JIntArray<'l>,
    sample_rate: jint,
) -> jshortArray {
    let pcm = guarded(|| {
        let n = env.get_array_length(&parts).ok()?;
        let mut list: Vec<Vec<f32>> = Vec::with_capacity(n as usize);
        for i in 0..n {
            let obj: JObject = env.get_object_array_element(&parts, i).ok()?;
            let arr = JFloatArray::from(obj);
            let len = env.get_array_length(&arr).ok()? as usize;
            let mut buf = vec![0f32; len];
            env.get_float_array_region(&arr, 0, &mut buf).ok()?;
            list.push(buf);
        }
        let glen = env.get_array_length(&gaps).ok()? as usize;
        let mut g = vec![0i32; glen];
        env.get_int_array_region(&gaps, 0, &mut g).ok()?;
        Some(bridge::join_pcm16(&list, &g, sample_rate))
    });
    let Some(pcm) = pcm else { return null_mut() };
    let Ok(arr) = env.new_short_array(pcm.len() as jint) else { return null_mut() };
    if env.set_short_array_region(&arr, 0, &pcm).is_err() {
        return null_mut();
    }
    arr.into_raw()
}
