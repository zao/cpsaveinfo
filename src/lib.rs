use byteorder::{ReadBytesExt, LE};
use log::{info, trace, warn};
use std::convert::TryInto;
use std::io::{Cursor, Read, Seek, SeekFrom};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;

struct Model {
    link: ComponentLink<Self>,
    value: i64,
}

enum Msg {
    AllowDrop(DragEvent),
    DoDrop(DragEvent),
}

trait ReadCDPRExt {
    fn read_packed_int(&mut self) -> std::io::Result<i64>;
    fn read_pstr(&mut self) -> std::io::Result<String>;
}

impl<T> ReadCDPRExt for T where T : Read {
    fn read_packed_int(&mut self) -> std::io::Result<i64> {
        let a = self.read_u8()?;
        let mut val = (a & 0x3F) as i64;
        let sign = a >= 0x80;
        if (a & 0x40) != 0 {
            let a = self.read_u8()? as i64;
            val |= (a & 0x7F) << 6;
            if a >= 0x80 {
                let a = self.read_u8()? as i64;
                val |= (a & 0x7F) << 13;
                if a >= 0x80 {
                    let a = self.read_u8()? as i64;
                    val |= (a & 0x7F) << 20;
                    if a >= 0x80 {
                        let a = self.read_u8()? as i64;
                        val |= a << 27;
                    }
                }
            }
        }
        Ok(if sign { -val } else { val })
    }

    fn read_pstr(&mut self) -> std::io::Result<String> {
        use std::io::{Error, ErrorKind};
        let count = self.read_packed_int()?;
        if count < 0 {
            let mut buf = vec![0u8; -count as usize];
            self.read_exact(&mut buf)?;
            String::from_utf8(buf).map_err(|e| Error::new(ErrorKind::InvalidData, e))
        }
        else {
            let mut buf = vec![0u16; count as usize];
            self.read_u16_into::<LE>(&mut buf)?;
            String::from_utf16(&buf).map_err(|e| Error::new(ErrorKind::InvalidData, e))
        }
    }
}

async fn read_save_structure(payload: &[u8]) -> Option<()> {
    let mut input = Cursor::new(&payload);
    input.seek(SeekFrom::End(-8)).ok()?;
    let tree_offset = input.read_u32::<LE>().ok()?;
    let mut sig_buf = [0u8; 4];
    input.read_exact(&mut sig_buf).ok()?;
    if &sig_buf != b"ENOD" {
        return None;
    }

    input.seek(SeekFrom::Start(tree_offset as u64)).ok()?;
    info!("tree offset: {}", tree_offset);
    input.read_exact(&mut sig_buf).ok()?;
    if &sig_buf != b"EDON" {
        return None;
    }
    let node_count = input.read_packed_int().ok()?;
    info!("node count: {}", node_count);
    for _ in 0..node_count {
        let name = input.read_pstr().ok()?;
        let next_idx = input.read_i32::<LE>().ok()?;
        let child_idx = input.read_i32::<LE>().ok()?;
        let data_offset = input.read_u32::<LE>().ok()?;
        let data_size = input.read_u32::<LE>().ok()?;
        info!("{:?}", (name, next_idx, child_idx, data_offset, data_size));
    }
    Some(())
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();
    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            value: 0,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::AllowDrop(e) => e.prevent_default(),
            Msg::DoDrop(e) => {
                info!("dropped");
                e.prevent_default();
                let dt = e.data_transfer().unwrap();
                let files = dt.files().unwrap();
                info!("{} files in drag list", files.length());
                for i in 0..files.length() {
                    let file = files.item(i).unwrap();
                    info!("{}: {:?}", i, file);
                    let size = file.size() as i64;
                    let buf_future = wasm_bindgen_futures::JsFuture::from(file.array_buffer());
                    wasm_bindgen_futures::spawn_local(async move {
                        match buf_future.await {
                            Ok(buf) => {
                                let typebuf = js_sys::Uint8Array::new(&buf);
                                let payload = typebuf.to_vec();
                                let save = read_save_structure(&payload).await;
                            }
                            _ => {
                                warn!("Could not read file");
                            }
                        }
                    });
                }
            }
        }
        true
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // Should only return "true" if new properties are different to
        // previously received properties.
        // This component has no properties so we will always return "false".
        false
    }

    fn view(&self) -> Html {
        html! {
            <>
            <h1
                ondragover=self.link.callback(|e| Msg::AllowDrop(e)),
                ondrop=self.link.callback(|e| Msg::DoDrop(e)),
            >
                {"Drag savegame here"}
            </h1>
            </>
        }
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    wasm_logger::init(wasm_logger::Config::default());
    App::<Model>::new().mount_to_body();
}