use napi::bindgen_prelude::*;
use napi_derive::napi;

/// module registration is done by the runtime, no need to explicitly do it now.
#[napi]
pub fn fibonacci(n: u32) -> u32 {
  match n {
    1 | 2 => 1,
    _ => fibonacci(n - 1) + fibonacci(n - 2),
  }
}

/// use `Fn`, `FnMut` or `FnOnce` traits to defined JavaScript callbacks
/// the return type of callbacks can only be `Result`.
#[napi]
pub fn get_cwd<T: Fn(String) -> Result<()>>(callback: T) {
  callback(
    std::env::current_dir()
      .unwrap()
      .to_string_lossy()
      .to_string(),
  )
  .unwrap();
}

/// or, define the callback signature in where clause
#[napi]
pub fn test_callback<T>(callback: T) -> Result<()>
where
  T: Fn(String) -> Result<()>,
{
  callback(std::env::current_dir()?.to_string_lossy().to_string())
}

// async fn, require `async` feature enabled.
// [dependencies]
// napi = {version="2", features=["async"]}
// #[napi]
// pub async fn read_file_async(path: String) -> Result<Buffer> {
//   Ok(tokio::fs::read(path).await?.into())
// }
