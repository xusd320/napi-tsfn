use std::marker::PhantomData;

use anyhow::Result;
use napi::bindgen_prelude::{spawn, FromNapiValue, JsValuesTupleIntoVec, Promise};
use napi::sys::{napi_env, napi_value};
use napi::threadsafe_function::{
  ErrorStrategy, ThreadsafeFunction as RawThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::{Env, JsFunction, JsUnknown, NapiRaw};
use napi_derive::napi;
use oneshot::channel;

pub struct ThreadsafeFunction<P: 'static, R> {
  raw: RawThreadsafeFunction<P, ErrorStrategy::Fatal>,
  env: napi_env,
  _phantom: PhantomData<R>,
}

impl<P: 'static + JsValuesTupleIntoVec, R> FromNapiValue for ThreadsafeFunction<P, R> {
  unsafe fn from_napi_value(env: napi_env, napi_val: napi_value) -> napi::Result<Self> {
    let raw = RawThreadsafeFunction::from_napi_value(env, napi_val)?;
    Ok(Self {
      raw,
      env,
      _phantom: PhantomData,
    })
  }
}

impl<P: 'static, R> Clone for ThreadsafeFunction<P, R> {
  fn clone(&self) -> Self {
    Self {
      raw: self.raw.clone(),
      env: self.env,
      _phantom: PhantomData,
    }
  }
}

unsafe impl<T: 'static, R> Sync for ThreadsafeFunction<T, R> {}
unsafe impl<T: 'static, R> Send for ThreadsafeFunction<T, R> {}

impl<P: 'static, R: 'static + Send + FromNapiValue> ThreadsafeFunction<P, R> {
  pub fn call(&self, value: P) -> Result<R> {
    let (sender, receiver) = channel();
    self.raw.call_with_return_value(
      value,
      ThreadsafeFunctionCallMode::NonBlocking,
      move |r: JsUnknown| {
        if r.is_promise().unwrap() {
          let promise = Promise::<R>::from_unknown(r).unwrap();
          spawn(async move {
            let r = promise.await.unwrap();
            sender.send(r).expect("Failed to send napi returned value.");
          });
        } else {
          let r = R::from_unknown(r).unwrap();
          sender.send(r).expect("Failed to send napi returned value.");
        }
        Ok(())
      },
    );
    let ret = receiver
      .recv()
      .expect("Failed to receive napi returned value.");
    Ok(ret)
  }
}

#[napi]
pub fn run(env: Env, func: JsFunction) -> String {
  let tsfn = unsafe {
    ThreadsafeFunction::<String, String>::from_napi_value(env.raw(), func.raw()).unwrap()
  };
  std::thread::spawn(move || {
    let ret = tsfn.call("1".to_string()).unwrap();

    println!("ret {} ", ret);
  });
  "x".to_string()
}
