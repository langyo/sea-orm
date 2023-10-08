use anyhow::Result;
use bytes::Bytes;

use wasmtime::{
    component::{Component, Linker},
    Engine, Module, Store, StoreLimits,
};
use wasmtime_wasi::preview2::{
    command, HostOutputStream, IsATTY, OutputStreamError, Table, WasiCtx, WasiCtxBuilder, WasiView,
};

lazy_static::lazy_static! {
    static ref STORE: Option<Store<WasiCtx>> = None;
}

struct VirtualOutputFile {
    data: Vec<u8>,
}

impl VirtualOutputFile {
    fn new() -> Self {
        Self { data: Vec::new() }
    }
}

#[async_trait::async_trait]
impl HostOutputStream for VirtualOutputFile {
    fn write(&mut self, bytes: Bytes) -> Result<(), OutputStreamError> {
        self.data.extend_from_slice(&bytes);
        println!("write: {:?}", String::from_utf8_lossy(&bytes));
        Ok(())
    }

    fn flush(&mut self) -> Result<(), OutputStreamError> {
        println!("flush");
        Ok(())
    }

    async fn write_ready(&mut self) -> Result<usize, OutputStreamError> {
        println!("write_ready");
        Ok(256)
    }
}
pub(crate) struct RunnerHostCtx {
    pub(crate) wasi: WasiCtx,
    pub(crate) limits: StoreLimits,
    pub(crate) table: Table,
}

impl WasiView for RunnerHostCtx {
    fn table(&self) -> &Table {
        &self.table
    }
    fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }
    fn ctx(&self) -> &WasiCtx {
        &self.wasi
    }
    fn ctx_mut(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let engine = Engine::default();
    let module = Module::from_file(&engine, "./target/wasm32-wasi/debug/api.wasm")?;
    let mut table = Table::new();

    let wasi = WasiCtxBuilder::new()
        .stdout(VirtualOutputFile::new(), IsATTY::No)
        .build(&mut table)?;
    let context = RunnerHostCtx {
        wasi,
        limits: StoreLimits::default(),
        table,
    };

    let mut linker = Linker::<RunnerHostCtx>::new(&engine);
    let mut store = Store::new(&engine, context);
    command::add_to_linker(&mut linker)?;

    let component = Component::from_utf8_lossy(
        r#"
package seaorm:demo

world proxy {
  export query: func(msg: string) -> result<string>
}
    "#,
    );
    let (wasi_cmd, _instance) =
        command::Command::instantiate_async(&mut store, &component, &linker).await?;
    let r = wasi_cmd.call(&mut store).await?;
    println!("Result: {:?}", r);

    Ok(())
}
